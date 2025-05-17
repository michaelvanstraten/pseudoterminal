use std::cell::OnceCell;
use std::fmt;

use std::io::{PipeReader, PipeWriter};
#[cfg(unix)]
use std::os::fd::OwnedFd;
#[cfg(windows)]
use std::os::windows::io::OwnedHandle;
use std::process::{ChildStdin as StdChildStdin, ChildStdout as StdChildStdout};

use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::process::{Child as ChildProcess, ChildStdin, ChildStdout, Command};

use crate::TerminalSize;
use crate::pal as imp;

/// Extension trait that adds pseudo-terminal spawning capabilities to `tokio::process::Command`.  
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
    /// use tokio::process::Command;
    /// use pseudoterminal::non_blocking::CommandExt;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut cmd = Command::new("bash");
    ///     let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
    ///
    ///     // Now you can interact with the terminal asynchronously
    ///     // ...
    /// }
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
    /// use tokio::process::Command;
    /// use pseudoterminal::{non_blocking::CommandExt, TerminalSize};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut cmd = Command::new("bash");
    ///     let size = TerminalSize {
    ///         columns: 100,
    ///         rows: 30,
    ///     };
    ///
    ///     let mut terminal = cmd.spawn_terminal_with_size(size)
    ///         .expect("Failed to spawn terminal");
    ///
    ///     // Now you can interact with the terminal asynchronously
    ///     // ...
    /// }
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
        self.spawn_terminal_with_size(TerminalSize::default())
    }

    fn spawn_terminal_with_size(&mut self, size: TerminalSize) -> io::Result<Terminal> {
        let maybe_handle = OnceCell::new();
        let maybe_io = OnceCell::new();
        let child_proc = self.spawn_with(|std_cmd| {
            let (handle, child_proc, io) = imp::Terminal::spawn_terminal(std_cmd, size)?;

            maybe_handle
                .set(handle)
                .expect("Cell has not been initialized");
            maybe_io.set(io).expect("Cell has not been initialized");

            Ok(child_proc)
        })?;

        Terminal::try_from((
            maybe_handle.into_inner().unwrap(),
            child_proc,
            maybe_io.into_inner().unwrap(),
        ))
    }
}

/// Represents an asynchronous pseudo-terminal (PTY).
///
/// This struct manages the PTY lifecycle, provides access to its I/O streams,
/// and allows interaction with the spawned child process asynchronously.
pub struct Terminal {
    handle: imp::Terminal,
    child_proc: ChildProcess,

    /// The input stream (write) for the terminal. `None` if split.
    pub terminal_in: Option<TerminalIn>,
    /// The output stream (read) for the terminal. `None` if split.
    pub terminal_out: Option<TerminalOut>,
}

impl TryFrom<(imp::Terminal, ChildProcess, (PipeWriter, PipeReader))> for Terminal {
    type Error = io::Error;

    fn try_from(
        (handle, child_proc, (writer, reader)): (
            imp::Terminal,
            ChildProcess,
            (PipeWriter, PipeReader),
        ),
    ) -> io::Result<Terminal> {
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                let std_child_input = <StdChildStdin as From<OwnedFd>>::from(writer.into());
                let std_child_output = <StdChildStdout as From<OwnedFd>>::from(reader.into());
            } else if #[cfg(windows)] {
                let std_child_input = <StdChildStdin as From<OwnedHandle>>::from(writer.into());
                let std_child_output = <StdChildStdout as From<OwnedHandle>>::from(reader.into());
            } else {
                panic!("Unsupported platform")
            }
        }

        let terminal_in = Some(TerminalIn {
            inner: ChildStdin::from_std(std_child_input)?,
        });

        let terminal_out = Some(TerminalOut {
            inner: ChildStdout::from_std(std_child_output)?,
        });

        Ok(Terminal {
            handle,
            child_proc,
            terminal_in,
            terminal_out,
        })
    }
}

impl Terminal {
    /// Splits the terminal into its input (`TerminalIn`) and output (`TerminalOut`) streams.
    ///
    /// This method consumes the terminal's I/O streams and returns them to the caller,
    /// removing them from the Terminal struct. After calling this method, attempting to
    /// access the terminal's I/O streams directly will return `None`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::process::Command;
    /// use pseudoterminal::non_blocking::CommandExt;
    /// use tokio::io::{AsyncWriteExt, AsyncReadExt};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut cmd = Command::new("bash");
    ///     let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
    ///     
    ///     if let Some((mut input, mut output)) = terminal.split() {
    ///         input.write_all(b"echo Hello, World!\n").await.expect("Write failed");
    ///         input.flush().await.expect("Flush failed");
    ///     
    ///         let mut buffer = String::new();
    ///         output.read_to_string(&mut buffer).await.expect("Read failed");
    ///         println!("Terminal output: {}", buffer);
    ///     }
    /// }
    /// ```
    pub fn split(&mut self) -> Option<(TerminalIn, TerminalOut)> {
        if self.terminal_in.is_none() || self.terminal_out.is_none() {
            return None;
        }

        let terminal_in = self.terminal_in.take().unwrap();
        let terminal_out = self.terminal_out.take().unwrap();

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
    /// use tokio::process::Command;
    /// use pseudoterminal::non_blocking::CommandExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let mut cmd = Command::new("bash");
    ///     let terminal = cmd.spawn_terminal()?;
    ///
    ///     let size = terminal.get_term_size()?;
    ///     println!("Terminal size: {} rows by {} columns", size.rows, size.columns);
    ///     Ok(())
    /// }
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
    /// - `new_size`: A `TerminalSize` struct specifying the desired dimensions.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::process::Command;
    /// use pseudoterminal::{non_blocking::CommandExt, TerminalSize};
    ///
    /// #[tokio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let mut cmd = Command::new("bash");
    ///     let mut terminal = cmd.spawn_terminal()?;
    ///
    ///     // Resize the terminal to 100 columns by 30 rows
    ///     let new_size = TerminalSize {
    ///         columns: 100,
    ///         rows: 30,
    ///     };
    ///     terminal.set_term_size(new_size)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if setting the terminal size fails.
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
    /// use tokio::process::Command;
    /// use pseudoterminal::non_blocking::CommandExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let mut cmd = Command::new("bash");
    ///     let terminal = cmd.spawn_terminal()?;
    ///
    ///     // Do some work with the terminal...
    ///
    ///     // Close the terminal when finished
    ///     terminal.close().await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// This function will return an error if killing the child process fails.
    pub async fn close(mut self) -> io::Result<()> {
        self.child_proc.kill().await
    }
}

/// An asynchronous input stream for writing to a pseudo-terminal.
///
/// This struct represents the input side of a pseudo-terminal and implements the
/// tokio `AsyncWrite` trait, allowing you to send data to the process running in the
/// terminal asynchronously. It wraps a platform-specific implementation to provide a consistent
/// interface across different operating systems.
///
/// # Examples
///
/// ```no_run
/// use tokio::process::Command;
/// use pseudoterminal::non_blocking::CommandExt;
/// use tokio::io::AsyncWriteExt;
///
/// #[tokio::main]
/// async fn main() {
///     let mut cmd = Command::new("bash");
///     let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
///
///     // Write a command to the terminal
///     if let Some(ref mut terminal_in) = terminal.terminal_in {
///         terminal_in.write_all(b"echo Hello, World!\n").await.expect("Write failed");
///         terminal_in.flush().await.expect("Flush failed");
///     }
/// }
/// ```
pub struct TerminalIn {
    inner: ChildStdin,
}

/// An asynchronous output stream for reading from a pseudo-terminal.
///
/// This struct represents the output side of a pseudo-terminal and implements the
/// tokio `AsyncRead` trait, allowing you to receive data from the process running in the
/// terminal asynchronously. It wraps a platform-specific implementation to provide a consistent
/// interface across different operating systems.
///
/// # Examples
///
/// ```no_run
/// use tokio::process::Command;
/// use pseudoterminal::non_blocking::CommandExt;
/// use tokio::io::{AsyncReadExt, AsyncWriteExt};
///
/// #[tokio::main]
/// async fn main() {
///     let mut cmd = Command::new("bash");
///     let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");
///
///     // Send a command
///     if let Some(ref mut terminal_in) = terminal.terminal_in {
///         terminal_in.write_all(b"echo Hello, World!\n").await.expect("Write failed");
///     }
///
///     // Read the output
///     if let Some(ref mut terminal_out) = terminal.terminal_out {
///         let mut buffer = [0; 1024];
///         let bytes_read = terminal_out.read(&mut buffer[..]).await.expect("Read failed");
///         println!("Read {} bytes: {}", bytes_read,
///                  String::from_utf8_lossy(&buffer[..bytes_read]));
///     }
/// }
/// ```
pub struct TerminalOut {
    inner: ChildStdout,
}

// Implement AsyncWrite for TerminalIn
impl AsyncWrite for TerminalIn {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        AsyncWrite::poll_write(std::pin::Pin::new(&mut self.inner), cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        AsyncWrite::poll_flush(std::pin::Pin::new(&mut self.inner), cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        AsyncWrite::poll_shutdown(std::pin::Pin::new(&mut self.inner), cx)
    }
}

// Implement AsyncRead for TerminalOut
impl AsyncRead for TerminalOut {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        AsyncRead::poll_read(std::pin::Pin::new(&mut self.inner), cx, buf)
    }
}

// Add Debug implementations
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
            .field("handle", &"<terminal_handle>")
            .field("child_proc", &self.child_proc)
            .field("terminal_in", &self.terminal_in)
            .field("terminal_out", &self.terminal_out)
            .finish()
    }
}
