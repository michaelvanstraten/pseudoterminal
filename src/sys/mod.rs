cfg_if::cfg_if! {
    if #[cfg(unix)] {
        mod unix;
        pub(crate) use unix::*;
    } else if #[cfg(windows)] {
        mod windows;
        pub use windows::*;
    }
}
