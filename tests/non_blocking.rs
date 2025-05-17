use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use pseudoterminal::{AsyncCommandExt, TerminalSize};

#[tokio::test]
async fn read_from_term() {
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            let mut cmd = Command::new("echo");
        } else if #[cfg(windows)] {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("echo");
        }
    }

    const TEST_STRING: &str = "Hello, World!";

    let mut terminal = cmd
        .arg(TEST_STRING)
        .spawn_terminal()
        .expect("should be spawnable");

    let mut buf = vec![0; TEST_STRING.len()];
    buf.resize(TEST_STRING.len(), 0);

    terminal
        .terminal_out
        .as_mut()
        .unwrap()
        .read_exact(&mut buf)
        .await
        .expect("terminal output was not readable");

    assert_eq!(buf, TEST_STRING.as_bytes());

    terminal.close().await.expect("failed to close terminal");
}

#[tokio::test]
async fn write_to_term() {
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            let mut cmd = Command::new("cat");
        } else if #[cfg(windows)] {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("findstr").arg("\"^\"");
        }
    }

    let mut terminal = cmd.spawn_terminal().expect("should be spawnable");

    const TEST_STRING: &str = "Hello, World!\r\n";

    terminal
        .terminal_in
        .as_mut()
        .unwrap()
        .write_all(TEST_STRING.as_bytes())
        .await
        .unwrap();

    let mut buf = vec![0; TEST_STRING.len()];
    buf.resize(TEST_STRING.len(), 0);

    terminal
        .terminal_out
        .as_mut()
        .unwrap()
        .read_exact(&mut buf)
        .await
        .expect("terminal output was not readable");

    assert_eq!(buf, TEST_STRING.as_bytes());

    terminal.close().await.expect("failed to close terminal");
}

#[tokio::test]
async fn set_term_size() {
    #[cfg(unix)]
    let mut cmd = Command::new("echo");
    #[cfg(windows)]
    let mut cmd = Command::new("cmd.exe");

    let mut terminal = cmd.spawn_terminal().expect("should be spawnable");

    let new_size = TerminalSize {
        columns: 40,
        rows: 60,
    };

    terminal
        .set_term_size(new_size)
        .expect("terminal size should be settable");

    #[cfg(unix)]
    assert_eq!(new_size, terminal.get_term_size().unwrap());

    terminal.close().await.expect("failed to close terminal");
}

#[tokio::test]
async fn split_terminal() {
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            let mut cmd = Command::new("cat");
        } else if #[cfg(windows)] {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("findstr").arg("\"^\"");
        }
    }

    let mut terminal = cmd.spawn_terminal().expect("should be spawnable");

    // Get the split streams
    let (mut terminal_in, mut terminal_out) =
        terminal.split().expect("should be able to split terminal");

    const TEST_STRING: &str = "Hello, Split Terminal!\r\n";

    terminal_in
        .write_all(TEST_STRING.as_bytes())
        .await
        .expect("should be able to write to split input");

    let mut buf = vec![0; TEST_STRING.len()];

    terminal_out
        .read_exact(&mut buf)
        .await
        .expect("should be able to read from split output");

    assert_eq!(buf, TEST_STRING.as_bytes());
}
