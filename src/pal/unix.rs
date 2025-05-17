use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child as ChildProcess, Command};

use nix::fcntl::FcntlArg::F_SETFD;
use nix::fcntl::{FcntlArg, FdFlag, OFlag as F, fcntl};
use nix::libc::{TIOCGWINSZ, TIOCSCTTY, TIOCSWINSZ, close, ioctl, setsid};
use nix::pty::{PtyMaster, Winsize, grantpt, posix_openpt, ptsname, unlockpt};

use crate::TerminalSize;

// Type definitions
pub type TerminalIn = File;
pub type TerminalOut = File;

// Core data structures
pub struct Terminal {
    master: PtyMaster,
}

pub struct TerminalIo {
    pub input: Option<TerminalIn>,
    pub output: Option<TerminalOut>,
}

// Terminal implementation
impl Terminal {
    pub fn spawn_terminal(
        cmd: &mut Command,
        initial_size: TerminalSize,
    ) -> io::Result<(Self, ChildProcess, TerminalIo)> {
        // Create master PTY
        let master = posix_openpt(F::O_RDWR | F::O_NOCTTY)?;
        grantpt(&master)?;
        unlockpt(&master)?;

        let raw_flags = fcntl(master.as_raw_fd(), FcntlArg::F_GETFD)?;
        let mut flags = FdFlag::from_bits_retain(raw_flags);
        flags |= FdFlag::FD_CLOEXEC;

        fcntl(master.as_raw_fd(), F_SETFD(flags))?;

        // Create separate file descriptors for input and output
        let master_fd = master.as_raw_fd();
        let input = unsafe { File::from_raw_fd(dup_fd(master_fd)?) };
        let output = unsafe { File::from_raw_fd(dup_fd(master_fd)?) };

        let terminal = Self { master };

        // Configure process to use the slave side of the PTY
        let slave = terminal.open_slave()?;

        cmd.stdin(slave.try_clone()?);
        cmd.stdout(slave.try_clone()?);
        cmd.stderr(slave);
        unsafe {
            cmd.pre_exec({
                move || {
                    // Close master in the child process
                    if close(master_fd) != 0 {
                        return Err(io::Error::last_os_error());
                    }

                    // Create a new session and set the slave as the controlling terminal
                    if setsid() < 0 {
                        return Err(io::Error::last_os_error());
                    }

                    if ioctl(0, TIOCSCTTY.into(), 1) != 0 {
                        return Err(io::Error::last_os_error());
                    }

                    Ok(())
                }
            })
        };

        terminal.set_term_size(initial_size)?;

        let child_proc = cmd.spawn()?;

        Ok((
            terminal,
            child_proc,
            TerminalIo {
                input: Some(input),
                output: Some(output),
            },
        ))
    }

    fn open_slave(&self) -> io::Result<OwnedFd> {
        let ptsname = unsafe { ptsname(&self.master) }?;
        let pts = OpenOptions::new().read(true).write(true).open(ptsname)?;
        Ok(pts.into())
    }

    #[cfg(feature = "non-blocking")]
    pub fn set_nonblocking(&self) -> io::Result<()> {
        let raw_flags = fcntl(self.master.as_raw_fd(), FcntlArg::F_GETFL)?;
        let mut flags = F::from_bits(raw_flags).expect("flags should be valid");
        flags |= F::O_NONBLOCK;

        fcntl(self.master.as_raw_fd(), FcntlArg::F_SETFL(flags))?;

        Ok(())
    }

    pub fn get_term_size(&self) -> io::Result<TerminalSize> {
        let mut winsz: Winsize = unsafe { std::mem::zeroed() };

        if unsafe { ioctl(self.master.as_raw_fd(), TIOCGWINSZ, &mut winsz as *mut _) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(TerminalSize {
            columns: winsz.ws_col,
            rows: winsz.ws_row,
        })
    }

    pub fn set_term_size(&self, new_size: TerminalSize) -> io::Result<()> {
        let winsz = Winsize::from(new_size);

        if unsafe { ioctl(self.master.as_raw_fd(), TIOCSWINSZ, &winsz) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

impl From<TerminalSize> for Winsize {
    fn from(value: TerminalSize) -> Self {
        Winsize {
            ws_row: value.rows,
            ws_col: value.columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }
}

fn dup_fd(fd: i32) -> io::Result<i32> {
    match nix::unistd::dup(fd) {
        Ok(new_fd) => Ok(new_fd),
        Err(e) => Err(io::Error::other(e)),
    }
}
