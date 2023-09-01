use std::{
    io::{Read, Write},
    process::Command,
};

use pseudoterminal::{CommandExt, TerminalSize};

#[test]
fn read_from_term() {
    let mut terminal = Command::new("echo")
        .arg("Hello, World!")
        .spawn_terminal()
        .expect("should be spawnable");

    let mut buf = String::new();

    terminal
        .termout
        .as_mut()
        .unwrap()
        .read_to_string(&mut buf)
        .expect("terminal output was not readable");

    assert_eq!(buf, "Hello, World!\r\n");

    terminal.close().expect("");
}

#[test]
fn write_to_term() {
    let mut terminal = Command::new("cat")
        .spawn_terminal()
        .expect("should be spawnable");

    const TEST_STRING: &str = "Hello, World!\r\n";

    terminal
        .termin
        .as_mut()
        .unwrap()
        .write_all(TEST_STRING.as_bytes())
        .unwrap();

    let mut buf = vec![0; TEST_STRING.len()];

    buf.resize(TEST_STRING.len(), 0);

    terminal
        .termout
        .as_mut()
        .unwrap()
        .read_exact(&mut buf)
        .expect("terminal output was not readable");

    assert_eq!(buf, TEST_STRING.as_bytes());

    terminal.close().expect("");
}

#[test]
fn set_term_size() {
    let mut terminal = Command::new("echo")
        .spawn_terminal()
        .expect("should be spawnable");

    let new_size = TerminalSize {
        columns: 40,
        rows: 60,
    };

    terminal
        .set_term_size(new_size)
        .expect("terminal size should be settable");

    assert_eq!(new_size, terminal.get_term_size().unwrap());

    terminal.close().expect("");
}
