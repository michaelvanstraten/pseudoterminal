#![feature(doc_auto_cfg)]
#![feature(windows_process_extensions_raw_attribute)]

mod blocking;
#[cfg(feature = "non-blocking")]
pub mod non_blocking;
mod sys;

pub use blocking::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalSize {
    pub rows: u16,
    pub columns: u16,
}
