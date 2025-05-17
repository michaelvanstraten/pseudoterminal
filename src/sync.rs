use std::fmt;
use std::io::{self, Read, Write};
#[cfg(CHANNEL_NIGHTLY)]
use std::io::{IoSlice, IoSliceMut};
use std::process::{Child as ChildProcess, Command};

use crate::TerminalSize;
use crate::pal as r#impl;

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
    /// let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
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
    ///     .expect("Failed to spawn terminal");
    ///
    /// // Now you can interact with the terminal
    /// // ...
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if the PTY creation fails, if setting up the
    /// process environment fails, or if spawning the process fails.
    fn spawn_terminal_with_size(&mut self, size: TerminalSize) -> io::Result<Terminal>;
}

impl CommandExt for Command {
    fn spawn_terminal(&mut self) -> io::Result<Terminal> {
        // Use a default terminal size
        let default_size = TerminalSize {
            rows: 24,
            columns: 80,
        };

        self.spawn_terminal_with_size(default_size)
    }

    fn spawn_terminal_with_size(&mut self, size: TerminalSize) -> io::Result<Terminal> {
        r#impl::Terminal::spawn_terminal(self, size).map(Terminal::from)
    }
}

/// Represents a synchronous pseudo-terminal (PTY).
///
/// This struct manages the PTY lifecycle, provides access to its I/O streams,
/// and allows interaction with the spawned child process.
pub struct Terminal {
    handle: r#impl::Terminal,
    child_proc: ChildProcess,
    /// The input stream (write) for the terminal. `None` if split.
    pub terminal_in: Option<TerminalIn>,
    /// The output stream (read) for the terminal. `None` if split.
    pub terminal_out: Option<TerminalOut>,
}

impl From<(r#impl::Terminal, ChildProcess, r#impl::TerminalIo)> for Terminal {
    fn from(
        (handle, child_proc, io): (r#impl::Terminal, ChildProcess, r#impl::TerminalIo),
    ) -> Terminal {
        Terminal {
            handle,
            child_proc,
            terminal_in: io.input.map(TerminalIn::from),
            terminal_out: io.output.map(TerminalOut::from),
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
    /// After calling this, `self.terminal_in` and `self.terminal_out` will be `None`.
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
    ///     input.write_all(b"echo hello\n").expect("Write failed");
    ///     
    ///     let mut buffer = String::new();
    ///     output.read_to_string(&mut buffer).expect("Read failed");
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
    /// terminal.set_term_size(new_size).expect("Failed to resize terminal");
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
    /// terminal.close().expect("Failed to close terminal");
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
/// let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
///
/// // Write a command to the terminal
/// if let Some(ref mut terminal_in) = terminal.terminal_in {
///     terminal_in.write_all(b"echo Hello, World!\n").expect("Write failed");
///     terminal_in.flush().expect("Flush failed");
/// }
/// ```
pub struct TerminalIn {
    inner: r#impl::TerminalIn,
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

impl AsRef<r#impl::TerminalIn> for TerminalIn {
    #[inline]
    fn as_ref(&self) -> &r#impl::TerminalIn {
        &self.inner
    }
}

impl From<TerminalIn> for r#impl::TerminalIn {
    fn from(val: TerminalIn) -> Self {
        val.inner
    }
}

impl From<r#impl::TerminalIn> for TerminalIn {
    fn from(pipe: r#impl::TerminalIn) -> TerminalIn {
        TerminalIn { inner: pipe }
    }
}

impl fmt::Debug for TerminalIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalIn").finish_non_exhaustive()
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
///     terminal_in.write_all(b"echo Hello, World!\n").expect("Write failed");
/// }
///
/// // Read the output
/// if let Some(ref mut terminal_out) = terminal.terminal_out {
///     let mut buffer = [0; 1024];
///     let bytes_read = terminal_out.read(&mut buffer).expect("Read failed");
///     println!("Read {} bytes: {}", bytes_read,
///              String::from_utf8_lossy(&buffer[..bytes_read]));
/// }
/// ```
pub struct TerminalOut {
    inner: r#impl::TerminalOut,
}

impl Read for TerminalOut {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    #[cfg(CHANNEL_NIGHTLY)]
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

impl AsRef<r#impl::TerminalOut> for TerminalOut {
    #[inline]
    fn as_ref(&self) -> &r#impl::TerminalOut {
        &self.inner
    }
}

impl From<TerminalOut> for r#impl::TerminalOut {
    fn from(val: TerminalOut) -> Self {
        val.inner
    }
}

impl From<r#impl::TerminalOut> for TerminalOut {
    fn from(pipe: r#impl::TerminalOut) -> TerminalOut {
        TerminalOut { inner: pipe }
    }
}

impl fmt::Debug for TerminalOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalOut").finish_non_exhaustive()
    }
}
