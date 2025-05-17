use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::{self, PipeReader, PipeWriter};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child as ChildProcess, Command};

use libc::{TIOCGWINSZ, TIOCSCTTY, TIOCSWINSZ, ioctl, setsid, winsize};

use crate::TerminalSize;

#[derive(Debug)]
pub struct Terminal {
    master: OwnedFd,
}

impl Terminal {
    pub fn spawn_terminal(
        cmd: &mut Command,
        initial_size: TerminalSize,
    ) -> io::Result<(Self, ChildProcess, (PipeWriter, PipeReader))> {
        let master = open_master()?;
        let slave = open_slave(&master)?;

        let terminal = Self { master };

        // Configure process to use the slave side of the PTY

        cmd.stdin(slave.try_clone()?);
        cmd.stdout(slave.try_clone()?);
        cmd.stderr(slave);
        unsafe {
            cmd.pre_exec({
                move || {
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

        let input = terminal.master.try_clone()?.into();
        let output = terminal.master.try_clone()?.into();

        Ok((terminal, child_proc, (input, output)))
    }

    pub fn get_term_size(&self) -> io::Result<TerminalSize> {
        let mut winsz: winsize = unsafe { std::mem::zeroed() };

        if unsafe { ioctl(self.master.as_raw_fd(), TIOCGWINSZ, &mut winsz as *mut _) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(TerminalSize {
            columns: winsz.ws_col,
            rows: winsz.ws_row,
        })
    }

    pub fn set_term_size(&self, new_size: TerminalSize) -> io::Result<()> {
        let winsz = winsize::from(new_size);

        if unsafe { ioctl(self.master.as_raw_fd(), TIOCSWINSZ, &winsz) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

impl From<TerminalSize> for winsize {
    fn from(value: TerminalSize) -> Self {
        winsize {
            ws_row: value.rows,
            ws_col: value.columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }
}

fn open_master() -> io::Result<OwnedFd> {
    let fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };

    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    if unsafe { libc::grantpt(fd) } != 0 {
        return Err(io::Error::last_os_error());
    }

    if unsafe { libc::unlockpt(fd) } != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn open_slave(master: &OwnedFd) -> io::Result<OwnedFd> {
    let name_ptr = unsafe { libc::ptsname(master.as_raw_fd()) };

    if name_ptr.is_null() {
        return Err(io::Error::last_os_error());
    }

    let name = unsafe { CStr::from_ptr(name_ptr) }
        .to_string_lossy()
        .into_owned();

    let pts = OpenOptions::new().read(true).write(true).open(name)?;

    Ok(pts.into())
}
