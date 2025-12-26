use std::fmt;
use std::io::{self, PipeReader, PipeWriter, Read, Write};
use std::io::{IoSlice, IoSliceMut};
use std::process::{Child as ChildProcess, Command};

use crate::TerminalSize;
use crate::pal as imp;

/// Extension trait that adds pseudo-terminal spawning capabilities to `std::process::Command`.  
///
/// This trait is automatically implemented for `Command` and provides methods to spawn
/// a new process with a pseudo-terminal as its controlling terminal.
pub trait CommandExt {
    /// Spawns a new process with a pseudo-terminal (PTY) as its controlling terminal.
    ///
    /// This method creates a new PTY and configures the process to use it as its
    /// standard input, output, and error streams. It uses a default terminal size
    /// of 80 columns by 24 rows.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use pseudoterminal::CommandExt;
    ///
    /// let mut cmd = Command::new("bash");
    /// let mut terminal = cmd.spawn_terminal().expect("spawning a pseudoterminal should succeed on supported platforms");
    ///
    /// // Now you can interact with the terminal
    /// // ...
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if the PTY creation fails, if setting up the
    /// process environment fails, or if spawning the process fails.
    fn spawn_terminal(&mut self) -> io::Result<Terminal>;

    /// Spawns a new process with a pseudo-terminal (PTY) as its controlling terminal,
    /// using the specified terminal size.
    ///
    /// This method creates a new PTY of the specified size and configures the process to use it
    /// as its standard input, output, and error streams.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use pseudoterminal::{CommandExt, TerminalSize};
    ///
    /// let mut cmd = Command::new("bash");
    /// let size = TerminalSize {
    ///     columns: 100,
    ///     rows: 30,
    /// };
    ///
    /// let mut terminal = cmd.spawn_terminal_with_size(size)
    ///     .expect("spawning a pseudoterminal with specific size should succeed on supported platforms");
    ///
    /// // Now you can interact with the terminal
    /// // ...
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if the PTY creation fails, if setting up the
    /// process environment fails, or if spawning the process fails.
    fn spawn_term_with_size(&mut self, size: TerminalSize) -> io::Result<Terminal>;
}

impl CommandExt for Command {
    fn spawn_terminal(&mut self) -> io::Result<Terminal> {
        self.spawn_term_with_size(TerminalSize::default())
    }

    fn spawn_term_with_size(&mut self, size: TerminalSize) -> io::Result<Terminal> {
        imp::Terminal::spawn_terminal(self, size).map(Terminal::from)
    }
}

/// Represents a synchronous pseudo-terminal (PTY).
///
/// This struct manages the PTY lifecycle, provides access to its I/O streams,
/// and allows interaction with the spawned child process.
///
/// `Terminal` implements `Read` and `Write` traits directly, allowing you to use it as a
/// standard I/O stream without explicitly accessing the inner streams. Read operations are
/// forwarded to the output stream, and write operations to the input stream.
///
/// ```no_run
/// use std::process::Command;
/// use pseudoterminal::CommandExt;
/// use std::io::{self, Read, Write};
///
/// fn main() -> io::Result<()> {
///     let mut cmd = Command::new("bash");
///     let mut terminal = cmd.spawn_terminal()?;
///
///     // Write directly to the terminal
///     terminal.write_all(b"echo Hello, World!\n")?;
///     terminal.flush()?;
///
///     // Read directly from the terminal
///     let mut buffer = [0; 1024];
///     let bytes_read = terminal.read(&mut buffer[..])?;
///     println!("Terminal output: {}", String::from_utf8_lossy(&buffer[..bytes_read]));
///
///     Ok(())
/// }
/// ```
///
/// If you've already called `split()` and taken away the terminal's I/O streams,
/// the `Read` and `Write` operations will return an appropriate `io::Error`.
pub struct Terminal {
    handle: imp::Terminal,
    child_proc: ChildProcess,
    /// The input stream (write) for the terminal. `None` if split.
    pub terminal_in: Option<TerminalIn>,
    /// The output stream (read) for the terminal. `None` if split.
    pub terminal_out: Option<TerminalOut>,
}

impl From<(imp::Terminal, ChildProcess, (PipeWriter, PipeReader))> for Terminal {
    fn from(
        (handle, child_proc, (input, output)): (
            imp::Terminal,
            ChildProcess,
            (PipeWriter, PipeReader),
        ),
    ) -> Terminal {
        Terminal {
            handle,
            child_proc,
            terminal_in: Some(input.into()),
            terminal_out: Some(output.into()),
        }
    }
}

impl Terminal {
    /// Splits the terminal into its input (`TerminalIn`) and output (`TerminalOut`) streams.
    ///
    /// This method consumes the terminal's I/O streams and returns them to the caller,
    /// allowing them to be used independently from the Terminal object itself.
    /// This is useful when you need to pass the streams to different parts of your
    /// application or handle them in separate threads.
    ///
    /// After calling this, `self.term_in` and `self.term_out` will be `None`.
    /// Returns `None` if the terminal has already been split.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use pseudoterminal::CommandExt;
    /// use std::io::{Read, Write};
    ///
    /// let mut cmd = Command::new("bash");
    /// let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
    ///
    /// // Split the terminal to handle I/O separately
    /// if let Some((mut input, mut output)) = terminal.split() {
    ///     // Now you can use input and output independently
    ///     input.write_all(b"echo hello\n").expect("writing to terminal should succeed unless the process has terminated");
    ///     
    ///     let mut buffer = String::new();
    ///     output.read_to_string(&mut buffer).expect("reading from terminal should succeed unless the process has terminated");
    ///     println!("Terminal output: {}", buffer);
    /// }
    /// ```
    pub fn split(&mut self) -> Option<(TerminalIn, TerminalOut)> {
        let terminal_in = self.terminal_in.take()?;
        let terminal_out = self.terminal_out.take()?;
        Some((terminal_in, terminal_out))
    }

    /// Gets the current size of the pseudo-terminal.
    ///
    /// Returns the terminal dimensions as a `TerminalSize` struct containing
    /// the number of rows and columns.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(unix)]
    /// # fn example() -> std::io::Result<()> {
    /// use std::process::Command;
    /// use pseudoterminal::CommandExt;
    ///
    /// let mut cmd = Command::new("bash");
    /// let terminal = cmd.spawn_terminal()?;
    ///
    /// let size = terminal.get_term_size()?;
    /// println!("Terminal size: {} rows by {} columns", size.rows, size.columns);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(unix)]
    pub fn get_term_size(&self) -> io::Result<TerminalSize> {
        self.handle.get_term_size()
    }

    /// Sets the size of the pseudo-terminal.
    ///
    /// This method resizes the terminal to the specified dimensions. The size change
    /// is communicated to the running program, which may redraw its output accordingly.
    ///
    /// # Parameters
    ///
    /// - `new_size`: The desired size for the terminal, containing rows and columns.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use pseudoterminal::{CommandExt, TerminalSize};
    ///
    /// let mut cmd = Command::new("bash");
    /// let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
    ///
    /// // Resize the terminal to 100x30
    /// let new_size = TerminalSize {
    ///     rows: 30,
    ///     columns: 100,
    /// };
    ///
    /// terminal.set_term_size(new_size).expect("resizing the terminal should succeed on supported platforms");
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if the terminal resizing operation fails.
    /// This can happen due to operating system limitations or if the provided dimensions
    /// are invalid for the underlying terminal implementation.
    pub fn set_term_size(&mut self, new_size: TerminalSize) -> io::Result<()> {
        self.handle.set_term_size(new_size)
    }

    /// Terminates the process running in the terminal and closes the terminal.
    ///
    /// This method will kill the child process and release any resources associated
    /// with the terminal. After calling this method, the terminal can no longer be used.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use pseudoterminal::CommandExt;
    ///
    /// let mut cmd = Command::new("bash");
    /// let terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
    ///
    /// // Do some work with the terminal...
    ///
    /// // When done, close the terminal and terminate the process
    /// terminal.close().expect("closing the terminal should succeed unless the process has already terminated");
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if killing the child process fails.
    pub fn close(mut self) -> io::Result<()> {
        self.child_proc.kill()?;

        Ok(())
    }
}

/// An input stream for writing to a pseudo-terminal.
///
/// This struct represents the input side of a pseudo-terminal and implements the
/// standard `Write` trait, allowing you to send data to the process running in the
/// terminal. It wraps a platform-specific implementation to provide a consistent
/// interface across different operating systems.
///
/// # Examples
///
/// ```no_run
/// use std::process::Command;
/// use pseudoterminal::CommandExt;
/// use std::io::Write;
///
/// let mut cmd = Command::new("bash");
/// let mut terminal = cmd.spawn_terminal().expect("spawning a pseudoterminal should succeed on supported platforms");
///
/// // Write a command to the terminal
/// if let Some(ref mut terminal_in) = terminal.terminal_in {
///     terminal_in.write_all(b"echo Hello, World!\n").expect("writing to terminal should succeed unless the process has terminated");
///     terminal_in.flush().expect("flushing terminal output should succeed unless the process has terminated");
/// }
/// ```
pub struct TerminalIn {
    inner: PipeWriter,
}

impl Write for TerminalIn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    #[cfg(CHANNEL_NIGHTLY)]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    #[cfg(CHANNEL_NIGHTLY)]
    #[inline]
    fn is_write_vectored(&self) -> bool {
        io::Write::is_write_vectored(&self.inner)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl From<PipeWriter> for TerminalIn {
    fn from(pipe: PipeWriter) -> TerminalIn {
        TerminalIn { inner: pipe }
    }
}

/// An output stream for reading from a pseudo-terminal.
///
/// This struct represents the output side of a pseudo-terminal and implements the
/// standard `Read` trait, allowing you to receive data from the process running in the
/// terminal. It wraps a platform-specific implementation to provide a consistent
/// interface across different operating systems.
///
/// # Examples
///
/// ```no_run
/// use std::process::Command;
/// use pseudoterminal::CommandExt;
/// use std::io::Read;
///
/// let mut cmd = Command::new("bash");
/// let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
///
/// // Send a command
/// if let Some(ref mut terminal_in) = terminal.terminal_in {
///     use std::io::Write;
///     terminal_in.write_all(b"echo Hello, World!\n").expect("writing to terminal should succeed unless the process has terminated");
/// }
///
/// // Read the output
/// if let Some(ref mut terminal_out) = terminal.terminal_out {
///     let mut buffer = [0; 1024];
///     let bytes_read = terminal_out.read(&mut buffer).expect("reading from terminal should succeed unless the process has terminated");
///     println!("Read {} bytes: {}", bytes_read,
///              String::from_utf8_lossy(&buffer[..bytes_read]));
/// }
/// ```
pub struct TerminalOut {
    inner: PipeReader,
}

impl Read for TerminalOut {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    #[cfg(CHANNEL_NIGHTLY)]
    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_to_end(buf)
    }
}

impl From<PipeReader> for TerminalOut {
    fn from(pipe: PipeReader) -> TerminalOut {
        TerminalOut { inner: pipe }
    }
}

impl fmt::Debug for TerminalIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalIn").finish_non_exhaustive()
    }
}

impl fmt::Debug for TerminalOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalOut").finish_non_exhaustive()
    }
}

impl fmt::Debug for Terminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Terminal")
            .field("child_proc", &self.child_proc)
            .field("term_in", &self.term_in)
            .field("term_out", &self.term_out)
            .finish_non_exhaustive()
    }
}

impl Read for Terminal {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.term_out {
            Some(out) => out.read(buf),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal output has been taken away",
            )),
        }
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        match &mut self.term_out {
            Some(out) => out.read_vectored(bufs),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal output has been taken away",
            )),
        }
    }

    #[cfg(CHANNEL_NIGHTLY)]
    #[inline]
    fn is_read_vectored(&self) -> bool {
        match &self.term_out {
            Some(out) => out.is_read_vectored(),
            None => false,
        }
    }
}

impl Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.term_in {
            Some(input) => input.write(buf),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal input has been taken away",
            )),
        }
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        match &mut self.term_in {
            Some(input) => input.write_vectored(bufs),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal input has been taken away",
            )),
        }
    }

    #[cfg(CHANNEL_NIGHTLY)]
    #[inline]
    fn is_write_vectored(&self) -> bool {
        match &self.term_in {
            Some(input) => input.is_write_vectored(),
            None => false,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.term_in {
            Some(input) => input.flush(),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal input has been taken away",
            )),
        }
    }
}
