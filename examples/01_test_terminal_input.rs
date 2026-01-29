//! Test and example for the terminal input bar implementation
//! run with: `cargo run --example test_terminal_input`

#![allow(unused_imports)]

use std::thread;
use std::time::Instant;

use mxs_serial_link::stdio_helper::*;
use mxs_serial_link::{terminal_exit, terminal_start};

fn main() -> io::Result<()> {
    let mut stdout = std::io::stdout();
    let mut serial_buffer = Vec::<&str>::new();

    terminal_start!();

    println! {"Press Ctrl+C to Exit"};
    println! {"Terminal size: {:?} \n", terminal::size().unwrap()};

    serial_buffer.push("------- Start --------\n");
    serial_buffer.push("Greetings\n ");
    serial_buffer.push("\nThis is ");
    serial_buffer.push("a test ");
    serial_buffer.push("too see where ");
    serial_buffer.push("characters are printed ");
    serial_buffer.push("as they are received from a simulated source\n");
    serial_buffer.push("Values:\n");
    serial_buffer.push("1\n2\n3");
    serial_buffer.push("\n4\n5\n");
    serial_buffer.push("----- End Stream -----\n\n");

    let input_prefix = "INPUT";
    let mut input = String::new();

    let mut last_print = Instant::now();
    let print_interval = Duration::from_millis(500);
    let mut msg_index = 0;

    loop {
        // Check for input (non-blocking)
        read_raw_stdin_input(&mut input).ok();

        // Detect new line in input buffer
        if input.ends_with('\n') {
            print!("\n{} {}", ">>:".green(), input.clone().blue());
            // Send to serial
            input.clear();
        }

        // Only print serial messages at intervals
        if last_print.elapsed() >= print_interval {
            let serial_msg = serial_buffer[msg_index];

            // Write buffer
            stdout.write(serial_msg.as_bytes()).ok();

            msg_index = if msg_index == serial_buffer.len() - 1 { 0 } else { msg_index + 1 };
            last_print = Instant::now();
        }

        // Update status bar with current input
        let status_bar_msg =
            format_args!("{} {} {}", input_prefix.red(), ">>:".green(), input.clone().blue())
                .to_string();
        print_input_bar(&status_bar_msg);

        thread::sleep(Duration::from_millis(10));
    }

    // End
    // terminal_exit!();
}
