use std::fs::File;
use std::io;
use std::mem::zeroed;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::process::CommandExt;
use std::os::windows::process::ProcThreadAttributeList;
use std::process::{Child, Command};

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE;

pub(crate) type AnonPipe = OwnedHandle;

pub(crate) fn spawn_terminal(cmd: &mut Command) -> io::Result<crate::sync::Terminal> {
    let (input_read_side, output_write_side) = unsafe { (zeroed(), zeroed()) };

    let terminal_handle = TermHandle::open(input_read_side, output_write_side)?;

    let proc_attrs = unsafe {
        ProcThreadAttributeList::build()
            .raw_attribute(
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                terminal_handle.0.0 as *const core::ffi::c_void,
                std::mem::size_of::<isize>(),
            )
            .finish()?
    };

    let child_proc = cmd.spawn_with_attributes(&proc_attrs)?;

    Ok(crate::sync::Terminal::new(
        child_proc,
        terminal_handle,
        (input_read_side, output_write_side),
    ))
}

pub struct TermHandle(HPCON);

impl TermHandle {
    fn open(input: HANDLE, output: HANDLE) -> io::Result<Self> {
        let size = COORD { X: 60, Y: 40 };

        let h_pc = unsafe { CreatePseudoConsole(size, input, output, 0)? };

        unsafe { CloseHandle(input)? };
        unsafe { CloseHandle(output)? };

        Ok(TermHandle(h_pc))
    }

    #[cfg(feature = "non-blocking")]
    pub fn set_nonblocking(&self) -> io::Result<()> {
        todo!()
    }

    pub fn set_term_size(&mut self, new_size: crate::TerminalSize) -> io::Result<()> {
        let coord_size = COORD {
            X: new_size.rows as i16,
            Y: new_size.columns as i16,
        };

        unsafe { Ok(ResizePseudoConsole(self.0, coord_size)?) }
    }
}

impl Drop for TermHandle {
    fn drop(&mut self) {
        unsafe { ClosePseudoConsole(self.0) }
    }
}
