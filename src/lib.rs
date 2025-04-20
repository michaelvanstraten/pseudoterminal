#![cfg_attr(all(doc, CHANNEL_NIGHTLY), feature(doc_auto_cfg))]
#![cfg_attr(windows, feature(windows_process_extensions_raw_attribute))]

pub mod sync;
#[cfg(feature = "non-blocking")]
pub mod r#async;
mod pal;


#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalSize {
    pub rows: u16,
    pub columns: u16,
}