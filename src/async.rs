use std::pin::Pin;
use std::process::Command as StdCommand;

use tokio::fs::File;
use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};

use crate::TerminalSize;
use crate::pal as r#impl;

pub trait CommandExt {
    fn spawn_terminal(&mut self) -> io::Result<Terminal>;
}

impl CommandExt for StdCommand {
    fn spawn_terminal(&mut self) -> io::Result<Terminal> {
        let (handle, (input_pipe, output_pipe)) = r#impl::create_terminal()?;

        #[cfg(feature = "non-blocking")]
        handle.set_nonblocking()?;

        // Create async Terminal with the command, handle, and I/O pipes
        let mut async_cmd = Command::from(self.clone());
        let child_proc = r#impl::spawn_terminal(&mut async_cmd.into_std())?.child_proc;
        let child = Child::from_std(child_proc);

        // Convert sync I/O to async I/O
        let tokio_input = File::from_std(input_pipe);
        let tokio_output = File::from_std(output_pipe);

        Ok(Terminal {
            handle,
            process: child,
            termin: Some(TerminalIn(tokio_input)),
            termout: Some(TerminalOut(tokio_output)),
        })
    }
}

pub struct Terminal {
    handle: r#impl::TermHandle,
    process: Child,
    pub termin: Option<TerminalIn>,
    pub termout: Option<TerminalOut>,
}

impl Terminal {
    /// Splits the terminal into its input (`TerminalIn`) and output (`TerminalOut`) streams.
    ///
    /// After calling this, `self.termin` and `self.termout` will be `None`.
    /// Returns `None` if the terminal has already been split.
    pub fn split(&mut self) -> Option<(TerminalIn, TerminalOut)> {
        let termin = self.termin.take()?;
        let termout = self.termout.take()?;
        Some((termin, termout))
    }

    /// Sets the size of the pseudo-terminal.
    pub fn set_term_size(&mut self, new_size: TerminalSize) -> io::Result<()> {
        self.handle.set_term_size(new_size)
    }

    /// Gets the size of the pseudo-terminal.
    #[cfg(unix)]
    pub fn get_term_size(&self) -> io::Result<TerminalSize> {
        self.handle.get_term_size()
    }

    /// Closes the terminal and terminates the child process.
    pub async fn close(mut self) -> io::Result<()> {
        self.process.kill().await?;
        Ok(())
    }
}

pub struct TerminalIn(File);

impl AsyncWrite for TerminalIn {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

pub struct TerminalOut(File);

impl AsyncRead for TerminalOut {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        dst: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_read(cx, dst)
    }
}
