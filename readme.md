# Pseudoterminal

The `pseudoterminal` crate is a versatile pseudoterminal (PTY) implementation
designed for Rust, offering both synchronous and asynchronous capabilities. This
library provides a straightforward and efficient means to interact with child
processes via pseudoterminals. Whether you're building interactive command-line
applications, custom terminals, or automating terminal interactions,
`pseudoterminal` is your reliable companion.

> [!WARNING]  
> The Asynchronous support is currently not implemented jet.

## Key Features

- **Cross-Platform Compatibility**: Works seamlessly on both Windows (using
  ConPTY) and Unix-based systems (using traditional PTY), ensuring broad
  compatibility for your projects.
- **Synchronous API**: Simple blocking I/O interface for straightforward
  terminal interaction.
- **Asynchronous Support**: Optional non-blocking I/O with the `non-blocking`
  feature, which integrates with asynchronous programming paradigms using
  libraries like Tokio.
- **Terminal Size Control**: Built-in methods for obtaining and modifying
  terminal dimensions, enabling dynamic terminal layout adjustments.

## Getting Started

### Installation

To include the `pseudoterminal` crate in your Rust project, simply add it as a
dependency in your `Cargo.toml`:

```toml
[dependencies]
pseudoterminal = "0.1.0"
```

### Example

Here's a basic example illustrating how to use the `pseudoterminal` crate to
spawn a terminal process and engage with it interactively:

```rust
use pseudoterminal::CommandExt;
use std::io::{stdin, stdout, Read, Write};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new pseudoterminal
    let mut cmd = Command::new("bash"); // Replace with your desired command
    let mut terminal = cmd.spawn_terminal()?;

    // Read from and write to the terminal
    let mut input_buffer = String::new();
    let mut output_buffer = [0u8; 1024];

    loop {
        // Read from user input or other sources
        stdin().read_line(&mut input_buffer)?;

        // Write input to the terminal
        terminal
            .terminal_in
            .as_mut()
            .unwrap()
            .write_all(input_buffer.as_bytes())?;

        // Read output from the terminal
        let bytes_read = terminal
            .terminal_out
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
```

## Documentation

For comprehensive documentation, including an in-depth API reference and
practical usage examples, please consult the official documentation at
[https://docs.rs/pseudoterminal](https://docs.rs/pseudoterminal).

## License

This crate is distributed under the permissive MIT License. For more details,
please review the [LICENSE](LICENSE) file.
