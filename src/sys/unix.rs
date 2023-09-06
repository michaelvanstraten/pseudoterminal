use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

use nix::fcntl::FcntlArg::F_SETFD;
use nix::fcntl::{fcntl, FcntlArg, FdFlag, OFlag as F};
use nix::libc::{close, ioctl, setsid, TIOCGWINSZ, TIOCSCTTY, TIOCSWINSZ};
use nix::pty::{grantpt, posix_openpt, ptsname, unlockpt, PtyMaster, Winsize};

pub(crate) fn open_handle_and_io(cmd: &mut Command) -> io::Result<(TerminalHandle, (File, File))> {
    let mut terminal_handle = TerminalHandle::open()?;

    let slave = terminal_handle.open_slave()?;

    cmd.stdin(slave.try_clone()?);
    cmd.stdout(slave.try_clone()?);
    cmd.stderr(slave);
    unsafe {
        cmd.pre_exec({
            let master = terminal_handle.0.as_raw_fd();
            move || {
                if close(master) != 0 {
                    return Err(io::Error::last_os_error());
                }

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

    let io = unsafe {
        (
            File::from_raw_fd(terminal_handle.0.as_raw_fd()),
            File::from_raw_fd(terminal_handle.0.as_raw_fd()),
        )
    };

    Ok((terminal_handle, io))
}

pub(crate) struct TerminalHandle(PtyMaster);

impl TerminalHandle {
    fn open() -> io::Result<Self> {
        let master = posix_openpt(F::O_RDWR | F::O_NOCTTY)?;
        grantpt(&master)?;
        unlockpt(&master)?;

        let raw_flags = fcntl(master.as_raw_fd(), FcntlArg::F_GETFD)?;
        let mut flags = FdFlag::from_bits_retain(raw_flags);
        flags |= FdFlag::FD_CLOEXEC;

        fcntl(master.as_raw_fd(), F_SETFD(flags))?;

        Ok(TerminalHandle(master))
    }

    fn open_slave(&mut self) -> io::Result<OwnedFd> {
        let ptsname = unsafe { ptsname(&self.0) }?;

        let pts = OpenOptions::new().read(true).write(true).open(ptsname)?;

        Ok(pts.into())
    }

    #[cfg(feature = "non-blocking")]
    pub fn set_nonblocking(&self) -> io::Result<()> {
        let raw_flags = fcntl(self.0.as_raw_fd(), FcntlArg::F_GETFD)?;
        let mut flags = F::from_bits(raw_flags).expect("flags should be valid");
        flags |= F::O_NONBLOCK;

        fcntl(self.0.as_raw_fd(), FcntlArg::F_SETFL(flags))?;

        Ok(())
    }

    pub fn get_term_size(&self) -> io::Result<crate::TerminalSize> {
        let mut winsz: Winsize = unsafe { std::mem::zeroed() };

        if unsafe { ioctl(self.0.as_raw_fd(), TIOCGWINSZ, &mut winsz as *mut _) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(crate::TerminalSize {
            columns: winsz.ws_col,
            rows: winsz.ws_row,
        })
    }

    pub fn set_term_size(&self, new_size: crate::TerminalSize) -> io::Result<()> {
        let winsz = Winsize::from(new_size);

        if unsafe { ioctl(self.0.as_raw_fd(), TIOCSWINSZ, &winsz) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    pub fn close(self) {}
}

impl From<crate::TerminalSize> for Winsize {
    fn from(value: crate::TerminalSize) -> Self {
        Winsize {
            ws_row: value.rows,
            ws_col: value.columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }
}
