#![cfg_attr(all(doc, CHANNEL_NIGHTLY), feature(doc_auto_cfg))]
#![cfg_attr(windows, feature(windows_process_extensions_raw_attribute))]
#![cfg_attr(CHANNEL_NIGHTLY, feature(can_vector))]

//! # Pseudoterminal
//!
//! A cross-platform Rust library for creating and managing pseudoterminals.
//!
//! This crate provides a unified interface for working with pseudoterminals (PTYs)
//! across different operating systems, allowing you to spawn processes within
//! a virtual terminal environment and interact with them programmatically.
//!
//! ## Example
//!
//! ```no_run
//! use std::process::Command;
//! use std::io::{Read, Write};
//! use pseudoterminal::CommandExt;
//!
//! fn main() -> std::io::Result<()> {
//!     // Create a command to run in the terminal
//!     let mut cmd = Command::new("bash");
//!    
//!     // Spawn the command in a pseudoterminal
//!     let mut terminal = cmd.spawn_terminal()?;
//!    
//!     // Write to the terminal
//!     if let Some(ref mut input) = terminal.terminal_in {
//!         input.write_all(b"echo Hello from pseudoterminal!\n")?;
//!         input.flush()?;
//!     }
//!    
//!     // Read from the terminal
//!     if let Some(ref mut output) = terminal.terminal_out {
//!         let mut buffer = [0; 1024];
//!         let bytes_read = output.read(&mut buffer)?;
//!         println!("{}", String::from_utf8_lossy(&buffer[..bytes_read]));
//!     }
//!    
//!     Ok(())
//! }
//! ```

#[cfg(windows)]
#[path = "pal/windows.rs"]
mod pal;

#[cfg(unix)]
#[path = "pal/unix.rs"]
mod pal;

pub mod blocking;

#[cfg(feature = "non-blocking")]
pub mod non_blocking;

pub use blocking::CommandExt;

#[cfg(feature = "non-blocking")]
pub use non_blocking::CommandExt as AsyncCommandExt;

/// Represents the dimensions of a terminal in rows and columns.
///
/// This struct is used when creating or resizing a pseudoterminal to
/// specify its dimensions.
///
/// # Examples
///
/// ```
/// use pseudoterminal::TerminalSize;
///
/// // Create a standard terminal size (80x24)
/// let size = TerminalSize {
///     rows: 24,
///     columns: 80,
/// };
///
/// // Create a larger terminal size
/// let large_size = TerminalSize {
///     rows: 50,
///     columns: 132,
/// };
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalSize {
    /// The number of rows (height) in the terminal
    pub rows: u16,
    /// The number of columns (width) in the terminal
    pub columns: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        TerminalSize {
            rows: 24,
            columns: 80,
        }
    }
}
