use std::pin::Pin;
use std::process::Command as StdCommand;

use tokio::fs::File;
use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};

use crate::sys::open_handle_and_io;
use crate::sys::TerminalHandle;

pub struct Terminal {
    handle: TerminalHandle,
    process: Child,
    pub termin: Option<TerminalIn>,
    pub termout: Option<TerminalOut>,
}

impl Terminal {
    pub(crate) fn new(
        cmd: StdCommand,
        handle: TerminalHandle,
        (termin, termout): (File, File),
    ) -> io::Result<Self> {
        let process = Command::from(cmd).spawn()?;

        Ok(Self {
            handle,
            process,
            termin: Some(TerminalIn(termin)),
            termout: Some(TerminalOut(termout)),
        })
    }

    pub async fn close(mut self) -> io::Result<()> {
        self.process.kill().await?;

        self.handle.close();

        Ok(())
    }
}

pub trait CommandExt {
    fn spawn_terminal(self) -> io::Result<Terminal>;
}

impl CommandExt for StdCommand {
    fn spawn_terminal(mut self) -> io::Result<Terminal> {
        let (handle, (termin, termout)) = open_handle_and_io(&mut self)?;

        handle.set_nonblocking()?;

        Terminal::new(self, handle, (termin.into(), termout.into()))
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
