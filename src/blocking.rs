use std::fs::File;
use std::io::{self, Read, Write};
use std::process::{Child, Command};

use crate::sys::{open_handle_and_io, TerminalHandle};
use crate::TerminalSize;

pub struct Terminal {
    handle: TerminalHandle,
    process: Child,
    pub termin: Option<TerminalIn>,
    pub termout: Option<TerminalOut>,
}

impl Terminal {
    pub(crate) fn new(
        cmd: &mut Command,
        handle: TerminalHandle,
        (termin, termout): (File, File),
    ) -> io::Result<Self> {
        let process = cmd.spawn()?;

        Ok(Self {
            handle,
            process,
            termin: Some(TerminalIn(termin)),
            termout: Some(TerminalOut(termout)),
        })
    }

    #[cfg(unix)]
    pub fn get_term_size(&mut self) -> io::Result<TerminalSize> {
        self.handle.get_term_size()
    }

    pub fn set_term_size(&mut self, new_size: TerminalSize) -> io::Result<()> {
        self.handle.set_term_size(new_size)
    }

    pub fn close(mut self) -> io::Result<()> {
        self.process.kill()?;

        self.handle.close();

        Ok(())
    }
}

pub trait CommandExt {
    fn spawn_terminal(&mut self) -> io::Result<Terminal>;
}

impl CommandExt for Command {
    fn spawn_terminal(&mut self) -> io::Result<Terminal> {
        let (handle, (termin, termout)) = open_handle_and_io(self)?;

        Terminal::new(self, handle, (termin, termout))
    }
}

pub struct TerminalIn(File);

impl Write for TerminalIn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_all(buf)
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> io::Result<()> {
        self.0.write_fmt(fmt)
    }
}

pub struct TerminalOut(File);

impl Read for TerminalOut {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.0.read_to_string(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_exact(buf)
    }
}
