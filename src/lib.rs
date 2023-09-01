#![feature(doc_auto_cfg)]
#![feature(stdio_makes_pipe)]

mod blocking;
#[cfg(feature = "non-blocking")]
pub mod non_blocking;
mod sys;

pub use blocking::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}
