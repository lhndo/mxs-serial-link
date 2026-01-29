//! Stdio Helper
//!
//! Handles Terminal init and de-init
//! Handles key input
//! Handles Ctrl+C hook
//! Enables displaying a persistent bottom input bar with history

#![allow(unused_must_use)]
#![allow(clippy::deref_addrof)]

pub use std::collections::VecDeque;
pub use std::io::{self, Write};
pub use std::time::Duration;

pub use crossterm::event::{self, Event, KeyCode};
pub use crossterm::style::Stylize;
pub use crossterm::{ExecutableCommand, QueueableCommand, cursor, terminal};

#[cfg(target_os = "linux")]
use termios::{ECHO, ICANON, TCSADRAIN, Termios};

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                             Globals
// —————————————————————————————————————————————————————————————————————————————————————————————————

pub const DEBUG: bool = false;
pub const TERM_PADDED_LINES: u16 = 2;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                             Macros
// —————————————————————————————————————————————————————————————————————————————————————————————————

#[macro_export]
macro_rules! terminal_start {
    () => {
        stdout_init();
    };
}

#[macro_export]
macro_rules! terminal_exit {
    () => {
        $crate::terminal_exit!(0);
    };
    ($code:expr) => {{
        stdout_de_init();
        if $code != 0 {
            println!("Exiting with code: {}\n", $code);
        }
        else {
            println!("Exiting...\n");
        }
        std::process::exit($code);
    }};
}

#[macro_export]
macro_rules! ctrl_c_init {
    () => {
        ctrlc::set_handler(move || {
            terminal_exit!();
        })
        .expect("Error setting Ctrl-C handler");
    };
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                            Functions
// —————————————————————————————————————————————————————————————————————————————————————————————————

/// Handling raw STD input with history
/// Example:
/// ```
/// let mut std_input = String::new();
/// ...
///
/// // In a loop:
/// read_stdin_input(&mut std_input)?;
///
/// // Detect new line in input buffer
/// if std_input.ends_with('\n') {
///     println!("{}", std_input) // Print the input line
///     std_input.clear();
/// }
/// ```
///  
pub fn read_raw_stdin_input(input: &mut String) -> Result<(), io::Error> {
    //
    const CTRL: event::KeyModifiers = event::KeyModifiers::CONTROL;

    static mut HISTORY: VecDeque<String> = VecDeque::<String>::new();
    static mut SCROLL_POS: usize = 0;

    while event::poll(Duration::from_millis(0))? {
        let event_in = event::read()?;

        // Local scope single entry point
        let history = unsafe { &mut *&raw mut HISTORY }; // Stops compiler yapping about mut static
        let scroll_pos = unsafe { &mut *&raw mut SCROLL_POS };

        if DEBUG {
            println!("\n>>> Event: {:?}", event_in); // Debug key events
        }

        if let Event::Key(key_event) = event_in {
            //
            if key_event.kind == event::KeyEventKind::Press {
                match (key_event.code, key_event.modifiers) {
                    // Ctrl-C
                    (KeyCode::Char('c'), CTRL) => {
                        terminal_exit!();
                    }
                    // Enter
                    (KeyCode::Enter, _) | (KeyCode::Char('j'), CTRL) => {
                        if history.front() != Some(input) && !input.is_empty() {
                            history.push_front(input.clone());
                        }
                        *scroll_pos = 0;
                        input.push('\n');
                    }
                    // Backspace
                    (KeyCode::Backspace, _) => {
                        input.pop();
                    }
                    // Ctrl + u - Clear
                    (KeyCode::Char('u'), CTRL) => {
                        input.clear();
                        *scroll_pos = 0;
                    }
                    // Up
                    (KeyCode::Up, _) => {
                        if let Some(item) = history.get(*scroll_pos) {
                            *input = item.clone();
                            *scroll_pos += 1;
                        }
                    }
                    // Down
                    (KeyCode::Down, _) => {
                        if *scroll_pos <= 1 {
                            input.clear();
                            *scroll_pos = 0;
                        }
                        else {
                            if let Some(item) = history.get(*scroll_pos - 2) {
                                *input = item.clone();
                                *scroll_pos -= 1;
                            }
                        }
                    }
                    // Esc
                    (KeyCode::Esc, _) => {
                        input.clear();
                        *scroll_pos = 0;
                    }
                    // Character Input
                    (KeyCode::Char(char), _) => {
                        input.push(char);
                    }
                    // Any
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

// —————————————————————————————————————————— Input Bar ————————————————————————————————————————————

/// Example:
/// ```
/// let input_prefix = "INPUT";
/// let mut input = String::new();
///
/// input = "Test Message".into();
///
/// let status_bar_msg =
///     format_args!("{} {} {}", input_prefix.red(), ">>:".green(), input.clone().blue())
///         .to_string();
/// print_input_bar(&status_bar_msg);
/// ```
///   
pub fn print_input_bar(status_bar_msg: &str) {
    let mut stdout = std::io::stdout();
    let (_cols, rows) = terminal::size().unwrap(); // Get current term size

    stdout.queue(cursor::SavePosition);
    stdout.queue(cursor::MoveTo(0, rows)); // Move to bottom
    stdout.queue(terminal::Clear(terminal::ClearType::CurrentLine)); // Clear

    stdout.write_all(status_bar_msg.as_bytes()); // Print status bar

    stdout.queue(cursor::MoveUp(TERM_PADDED_LINES)); // Move up to scroll region
    stdout.execute(cursor::RestorePosition);
}

// ———————————————————————————————————————————— Init ———————————————————————————————————————————————

/// Init Terminal
pub fn stdout_init() {
    ctrl_c_init!();

    // On Linux we disable canonical mode (instead of raw mode) to gain access to non buffered input
    #[cfg(target_os = "linux")]
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

    stdout.queue(cursor::Hide);
    stdout.queue(cursor::SavePosition);

    print!("\x1b[0m"); // Reset Style
    print!("{}", "\n".repeat(TERM_PADDED_LINES as usize + 1)); // PAD previous output
    print!("\x1b[r"); // Reset scrollable region
    print!("\x1b[{};{}r", 0, rows - TERM_PADDED_LINES); // Set scrollable region

    stdout.queue(cursor::RestorePosition);
    stdout.execute(cursor::MoveToRow(rows - TERM_PADDED_LINES - 1)); // Move to upper region
}

// ——————————————————————————————————————————— De-Init —————————————————————————————————————————————

// De-init Terminal
pub fn stdout_de_init() {
    let mut stdout = std::io::stdout();
    let (_cols, rows) = terminal::size().unwrap();

    crossterm::terminal::disable_raw_mode(); // Takes care of restoring termios canonical mode

    print!("\x1b[r"); // Reset scrollable region
    print!("\x1b[0m"); // Reset Style

    stdout.queue(cursor::MoveTo(0, rows)); // Move to bottom
    stdout.queue(terminal::Clear(terminal::ClearType::CurrentLine)); // Clear
    stdout.execute(cursor::Show);
}
