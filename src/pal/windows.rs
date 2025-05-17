use std::io;
use std::os::windows;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child as ChildProcess, Command};

use crate::TerminalSize;

pub struct Terminal {
    handle: HPCON,
}

pub type TerminalIn = io::PipeWriter;
pub type TerminalOut = io::PipeReader;

pub struct TerminalIo {
    pub input: Option<TerminalIn>,
    pub output: Option<TerminalOut>,
}

// Core Terminal implementation
impl Terminal {
    pub fn spawn_terminal(
        cmd: &mut Command,
        initial_size: TerminalSize,
    ) -> io::Result<(Self, ChildProcess, TerminalIo)> {
        let (input_read, input_write) = io::pipe()?;
        let (output_read, output_write) = io::pipe()?;

        let mut handle = unsafe { core::mem::zeroed() };

        unsafe extern "system" {
            fn CreatePseudoConsole(
                size: COORD,
                hinput: windows::raw::HANDLE,
                houtput: windows::raw::HANDLE,
                dwflags: u32,
                phpc: *mut HPCON,
            ) -> HRESULT;
        }

        cvt(unsafe {
            CreatePseudoConsole(
                COORD::from(initial_size),
                input_read.as_raw_handle(),
                output_write.as_raw_handle(),
                0,
                &mut handle,
            )
        })?;

        const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 131094;

        let proc_attrs = unsafe {
            windows::process::ProcThreadAttributeList::build()
                .raw_attribute(
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                    handle as *const core::ffi::c_void,
                    std::mem::size_of::<isize>(),
                )
                .finish()?
        };

        let child_proc = cmd.spawn_with_attributes(&proc_attrs)?;

        Ok((
            Self { handle },
            child_proc,
            TerminalIo {
                input: Some(input_write),
                output: Some(output_read),
            },
        ))
    }

    pub fn set_term_size(&self, new_size: TerminalSize) -> io::Result<()> {
        unsafe extern "system" {
            fn ResizePseudoConsole(hpc: HPCON, size: COORD) -> HRESULT;
        }

        unsafe { cvt(ResizePseudoConsole(self.handle, COORD::from(new_size))) }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        unsafe extern "system" {
            fn ClosePseudoConsole(hpc: HPCON);
        }

        unsafe { ClosePseudoConsole(self.handle) }
    }
}

type HPCON = isize;
type HRESULT = i32;

#[repr(C)]
#[allow(non_snake_case)]
pub struct COORD {
    pub X: i16,
    pub Y: i16,
}

impl From<TerminalSize> for COORD {
    fn from(size: TerminalSize) -> Self {
        COORD {
            X: size.rows as i16,
            Y: size.columns as i16,
        }
    }
}

pub fn cvt(res: HRESULT) -> io::Result<()> {
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
