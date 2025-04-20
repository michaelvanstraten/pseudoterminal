use std::fmt;
use std::io::{self, IoSlice, Read, Write};
use std::process::{Child as ChildProcess, Command};

use crate::pal as imp;
use crate::TerminalSize;

/// Represents a synchronous pseudo-terminal (PTY).
///
/// This struct manages the PTY lifecycle, provides access to its I/O streams,
/// and allows interaction with the spawned child process.
pub struct Terminal {
    handle: imp::TermHandle,
    child_proc: ChildProcess,
    /// The input stream (write) for the terminal. `None` if split.
    pub termin: Option<TermIn>,
    /// The output stream (read) for the terminal. `None` if split.
    pub termout: Option<TermOut>,
}

impl Terminal {
    pub(crate) fn new(
        child_proc: ChildProcess,
        handle: imp::TermHandle,
        (input_pipe, output_pipe): (imp::AnonPipe, imp::AnonPipe),
    ) -> Self {
        let termin = TermIn::from(input_pipe);
        let termout = TermOut::from(output_pipe);

        Terminal {
            handle,
            child_proc,
            termin: Some(termin),
            termout: Some(termout),
        }
    }

    /// Splits the terminal into its input (`TermIn`) and output (`TermOut`) streams.
    ///
    /// After calling this, `self.termin` and `self.termout` will be `None`.
    /// Returns `None` if the terminal has already been split.
    pub fn split(&mut self) -> Option<(TermIn, TermOut)> {
        let termin = self.termin.take()?;
        let termout = self.termout.take()?;
        Some((termin, termout))
    }

    /// Gets the size of the pseudo-terminal.
    ///
    /// Note: On Windows, this might not reflect the actual console buffer size
    /// but rather the size set by `set_term_size`. ConPTY size querying is limited.
    #[cfg(unix)]
    pub fn get_term_size(&self) -> io::Result<TerminalSize> { // Changed to &self
        self.handle.get_term_size()
    }

    /// Sets the size of the pseudo-terminal.
    pub fn set_term_size(&mut self, new_size: TerminalSize) -> io::Result<()> {
        self.handle.set_term_size(new_size)
    }

    pub fn close(mut self) -> io::Result<()> {
        self.child_proc.kill()?;

        Ok(())
    }
}

pub struct TermIn {
    inner: imp::AnonPipe,
}

impl Write for TermIn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&*self).write_vectored(bufs)
    }

    fn is_write_vectored(&self) -> bool {
        io::Write::is_write_vectored(&&*self)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}

impl Write for &TermIn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRef<imp::AnonPipe> for TermIn {
    #[inline]
    fn as_ref(&self) -> &imp::AnonPipe {
        &self.inner
    }
}

impl Into<imp::AnonPipe> for TermIn {
    fn into(self) -> imp::AnonPipe {
        self.inner
    }
}

impl From<imp::AnonPipe> for TermIn {
    fn from(pipe: imp::AnonPipe) -> TermIn {
        TermIn { inner: pipe }
    }
}


impl fmt::Debug for TermIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TermIn").finish_non_exhaustive()
    }
}


pub struct TermOut {
    inner: imp::AnonPipe,
}

impl Read for TermOut {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.inner.read_buf(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_to_end(buf)
    }
}

impl AsRef<imp::AnonPipe> for TermOut {
    #[inline]
    fn as_ref(&self) -> &imp::AnonPipe {
        &self.inner
    }
}

impl Into<imp::AnonPipe> for TermOut {
    fn into(self) -> imp::AnonPipe {
        self.inner
    }
}

impl From<imp::AnonPipe> for TermOut {
    fn from(pipe: imp::AnonPipe) -> TermOut {
        TermOut { inner: pipe }
    }
}

impl fmt::Debug for TermOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TermOut").finish_non_exhaustive()
    }
}

pub trait CommandExt {
    fn spawn_terminal(&mut self) -> io::Result<Terminal>;
}

impl CommandExt for Command {
    fn spawn_terminal(&mut self) -> io::Result<Terminal> {
        imp::spawn_terminal(self)
    }
}