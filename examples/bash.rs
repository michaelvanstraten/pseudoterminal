use pseudoterminal::CommandExt;
use std::io::{stdout, Read, Write};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new pseudoterminal
    let mut cmd = Command::new("bash"); // or any other desired command
    let mut terminal = cmd.spawn_terminal()?;

    // Read from and write to the terminal
    let mut input_buffer = String::new();
    let mut output_buffer = [0u8; 1024];

    loop {
        // Read from user input or other sources
        std::io::stdin().read_line(&mut input_buffer)?;

        // Write input to the terminal
        terminal
            .termin
            .as_mut()
            .unwrap()
            .write_all(input_buffer.as_bytes())?;

        // Read output from the terminal
        let bytes_read = terminal
            .termout
            .as_mut()
            .unwrap()
            .read(&mut output_buffer)?;

        // Write read bytes to stdout
        stdout().write_all(&output_buffer[..bytes_read])?;
        stdout().flush()?;

        // Clear the input buffer
        input_buffer.clear();
    }
}
