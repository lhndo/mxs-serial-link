#![allow(unused_must_use)]

pub use std::cell::Cell;
pub use std::collections::VecDeque;
pub use std::io::{self, Write};
pub use std::time::Duration;

pub use crossterm::event::{self, Event, KeyCode};
pub use crossterm::style::Stylize;
pub use crossterm::{ExecutableCommand, QueueableCommand, cursor, terminal};
use termios::{ECHO, ICANON, TCSADRAIN, Termios};

// fn main() -> io::Result<()> {
//   let mut stdout = std::io::stdout();
//   let mut serial_buffer = Vec::<&str>::new();

//   init_stdout();

//   println!("-------Start--------");
//   println! {"Terminal size: {:?}", terminal::size().unwrap()};
//   println!(">>|<<");

//   serial_buffer.push("Greetings\n ");
//   serial_buffer.push("\n\nThis is ");
//   serial_buffer.push("a test ");
//   serial_buffer.push("too see where ");
//   serial_buffer.push("characters are printed.\n");
//   serial_buffer.push("Value: ");
//   serial_buffer.push("23S\n1\n2\n3");
//   serial_buffer.push("\n4\n5\n");
//   serial_buffer.push("End\n\n\n");

//   let mut input = String::new();
//   let mut input_history = VecDeque::<String>::new();

//   for _ in 0..1111 {
//       // Read serial msg
//       for serial_msg in &serial_buffer {
//           thread::sleep(Duration::from_millis(300)); // Simulating io delay

//           // Non Blocking stdin read
//           stdin_read_raw(&mut input, &mut input_history);

//           // Detect new line in input buffer
//           if input.ends_with('\n') {
//               print!("\n{} {}", ">>:".green(), input.clone().blue());
//               // Send to serial
//               input.clear();
//           }

//           // Format status msg
//           let status_bar_msg =
//               format_args!("{} {} {}", "COM8".red(), ">>:".green(), input.clone().blue())
//                   .to_string();

//           // Write buffer
//           stdout.write(serial_msg.as_bytes());

//           // Print output with status bar
//           print_status_bar(&status_bar_msg);
//       }
//   }

//   // End
//   println!("Done \n");
//   de_init_stdout();
//   Ok(())
// }

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                             Globals
// —————————————————————————————————————————————————————————————————————————————————————————————————

pub const TERM_PAD: u16 = 2;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                             Macros
// —————————————————————————————————————————————————————————————————————————————————————————————————

#[macro_export]
macro_rules! exit_process {
    () => {
        $crate::exit_process!(0);
    };
    ($code:expr) => {{
        stdout_de_init();
        println!("\nExiting...\n");
        std::process::exit($code);
    }};
}

#[macro_export]
macro_rules! init_process {
    () => {
        stdout_init();
    };
}

#[macro_export]
macro_rules! ctrl_c_init {
    () => {
        ctrlc::set_handler(move || {
            exit_process!();
        })
        .expect("Error setting Ctrl-C handler");
    };
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                            Functions
// —————————————————————————————————————————————————————————————————————————————————————————————————

pub fn stdin_read_raw(
    input: &mut String,
    input_history: &mut VecDeque<String>,
) -> Result<(), io::Error> {
    //
    thread_local! {
        static SCROLL_POS: Cell<usize> = const { Cell::new(0)};
    }

    // Raw mode is needed to capture non buffered input (before <CR>)
    // terminal::enable_raw_mode();

    while event::poll(Duration::from_millis(0))? {
        let event_in = event::read()?;
        // println!("\n>>> Event: {:?}", event_in); // Debug key events
        if let Event::Key(key_event) = event_in {
            if key_event.kind == event::KeyEventKind::Press {
                // CTRL
                if key_event.modifiers == event::KeyModifiers::CONTROL {
                    match key_event.code {
                        KeyCode::Char(c) if c == ('c') => {
                            exit_process!();
                        }
                        KeyCode::Char(c) if c == ('j') => {
                            if input_history.front() != Some(input) {
                                input_history.push_front(input.clone());
                            }
                            SCROLL_POS.set(0);
                            input.push('\n');
                        }
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        _ => {}
                    }
                }
                else {
                    // Keys
                    match key_event.code {
                        KeyCode::Char(c) => input.push(c),
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Enter => {
                            if input_history.front() != Some(input) {
                                input_history.push_front(input.clone());
                            }
                            SCROLL_POS.set(0);
                            input.push('\n');
                        }
                        KeyCode::Up => {
                            let scroll_pos = SCROLL_POS.get();

                            if let Some(item) = input_history.get(scroll_pos) {
                                *input = item.clone();
                                SCROLL_POS.set(scroll_pos + 1);
                            }
                        }
                        KeyCode::Down => {
                            let scroll_pos = SCROLL_POS.get();

                            if scroll_pos <= 1 {
                                input.clear();
                                SCROLL_POS.set(0);
                            }
                            else {
                                if let Some(item) = input_history.get(scroll_pos - 1) {
                                    *input = item.clone();
                                    SCROLL_POS.set(scroll_pos - 1);
                                }
                            }
                        }
                        KeyCode::Esc => {
                            input.clear();
                            SCROLL_POS.set(0);
                        }

                        _ => {}
                    }
                }
            }
        }
    }
    // terminal::disable_raw_mode();

    Ok(())
}

pub fn print_input_bar(status_message: &str) {
    let mut stdout = std::io::stdout();
    let (_cols, rows) = terminal::size().unwrap(); // Get current term size

    stdout.queue(cursor::SavePosition);
    stdout.queue(cursor::MoveTo(0, rows)); // Move to bottom
    stdout.queue(terminal::Clear(terminal::ClearType::CurrentLine)); // Clear

    stdout.write_all(status_message.as_bytes()); // Print status bar

    stdout.queue(cursor::MoveUp(TERM_PAD)); // Move up to scroll region
    stdout.execute(cursor::RestorePosition);
}

/// Init Terminal
pub fn stdout_init() {
    ctrl_c_init!();

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        // Open the standard input file descriptor
        let fd = io::stdin().as_raw_fd();

        // Get the current terminal settings
        let mut termios = Termios::from_fd(fd).unwrap();

        // Disable canonical mode (ICANON) and echo (ECHO)
        // This is needed so we preserve the other terminal modes, but get non buffered input
        // events
        termios.c_lflag &= !(ICANON | ECHO);
        termios.c_cc[termios::VMIN] = 1; // Minimum number of characters to read
        termios.c_cc[termios::VTIME] = 0; // Timeout in deciseconds (0 means no timeout)

        // Apply
        termios::tcsetattr(fd, TCSADRAIN, &termios);
    }

    let mut stdout = std::io::stdout();
    let (_cols, rows) = terminal::size().unwrap();

    stdout.queue(cursor::SavePosition);
    stdout.queue(cursor::Hide);

    print!("\x1b[0m"); // Reset Style
    print!("{}", "\n".repeat(TERM_PAD as usize + 1)); // PAD previous output
    print!("\x1b[r"); // Reset scrollable region
    print!("\x1b[{};{}r", 0, rows - TERM_PAD); // Set scrollable region

    stdout.queue(cursor::RestorePosition);
    stdout.execute(cursor::MoveToRow(rows - TERM_PAD - 1)); // Move to upper region
}

// De-init Terminal
pub fn stdout_de_init() {
    let mut stdout = std::io::stdout();
    let (_cols, rows) = terminal::size().unwrap();

    crossterm::terminal::disable_raw_mode(); // Takes care of termios canonical mode

    print!("\x1b[r"); // Reset scrollable region
    print!("\x1b[0m"); // Reset Style

    stdout.queue(cursor::MoveTo(0, rows)); // Move to bottom
    stdout.queue(terminal::Clear(terminal::ClearType::CurrentLine)); // Clear
    stdout.execute(cursor::Show);
}
