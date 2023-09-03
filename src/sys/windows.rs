use std::fs::File;
use std::io;
use std::mem::zeroed;
use std::os::windows::io::FromRawHandle;
use std::os::windows::process::CommandExt;
use std::process::Command;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE;

pub(crate) fn open_handle_and_io(cmd: &mut Command) -> io::Result<(TerminalHandle, (File, File))> {
    // - Close these after CreateProcess of child application with pseudoconsole object.
    let (mut input_read_side, mut output_write_side) = unsafe { (zeroed(), zeroed()) };

    // - Hold onto these and use them for communication with the child through the pseudoconsole.
    let (mut output_read_side, mut input_write_side) = unsafe { (zeroed(), zeroed()) };

    unsafe {
        CreatePipe(&mut input_read_side, &mut input_write_side, None, 0)?;
        CreatePipe(&mut output_read_side, &mut output_write_side, None, 0)?;
    }

    let terminal_handle = TerminalHandle::open(input_read_side, output_write_side)?;

    unsafe {
        cmd.raw_attribute(
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            terminal_handle.0,
        )
    };

    let io = unsafe {
        (
            File::from_raw_handle(output_read_side.0 as *mut _),
            File::from_raw_handle(input_write_side.0 as *mut _),
        )
    };

    Ok((terminal_handle, io))
}

pub struct TerminalHandle(HPCON);

impl TerminalHandle {
    fn open(input: HANDLE, output: HANDLE) -> io::Result<Self> {
        let size = COORD { X: 60, Y: 40 };

        let h_pc = unsafe { CreatePseudoConsole(size, input, output, 0)? };

        unsafe { CloseHandle(input)? };
        unsafe { CloseHandle(output)? };

        Ok(TerminalHandle(h_pc))
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

    pub(crate) fn close(self) -> io::Result<()> {
        unsafe { ClosePseudoConsole(self.0) }

        Ok(())
    }
}
