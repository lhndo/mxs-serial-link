mod data;
mod mxs_decoder;
mod mxs_shared;
mod stdio_helper;

use std::env;
use std::io::Read;
use std::sync::{OnceLock, mpsc};
use std::thread::{self, JoinHandle, sleep};

use data::*;
use mxs_decoder::*;
use serialport::SerialPort;
use stdio_helper::*;

use anyhow::{Context, Result as AnyResult};

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                             Globals
// —————————————————————————————————————————————————————————————————————————————————————————————————

const TIMEOUT: Duration = Duration::from_millis(500);
const READ_BUFFER_SIZE: usize = 2000;

/// Direct mode skips MXS packet filtering
static DIRECT_MODE: OnceLock<bool> = OnceLock::new();

#[cfg(target_os = "linux")]
type PortType = serialport::TTYPort;
#[cfg(windows)]
type PortType = serialport::COMPort;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                              Main
// —————————————————————————————————————————————————————————————————————————————————————————————————

fn main() {
    // ——————————————————————————————————————— Stdio Init ——————————————————————————————————————————

    terminal_start!();

    // —————————————————————————————————————————— Args —————————————————————————————————————————————

    let args: Vec<String> = env::args().collect();

    // Print Help
    if args.contains(&"help".to_string()) {
        print!(
            r#" 
  MXS Serial Link - Serial Communication Program for Embedded Applications

    Usage: mxs [port] [options]

      Arguments:

        [port]   - port name. Defaults to largest port 
        direct   - direct mode. Skips MXP packet filtering 
        help     - displays this message 
           "#
        );
        terminal_exit!();
    }

    println!("\n=== Serial Link Started ===");

    // Direct mode skips MXS packet filtering
    let direct = if args.contains(&"direct".to_string()) {
        println!("        Direct mode \n");
        true
    }
    else {
        println!("     with MXS Protocol \n");
        false
    };

    DIRECT_MODE.set(direct).unwrap();

    // First argument should be the port name
    let mut input_port: String = args
        .get(1)
        .map(|s| if s != "direct" { s.to_string() } else { String::new() })
        .unwrap_or("".to_string());

    // ———————————————————————————————————————— Main Loop ——————————————————————————————————————————

    'main: loop {
        println!("\nAvailable Ports");
        println!("==============");
        if let Ok(ports) = serialport::available_ports() {
            for port in &ports {
                println!("{}", port.port_name.clone().dark_blue());
            }
        }
        else {
            println!("{}", "No ports".red())
        }
        println!("______________");

        if input_port.is_empty() {
            println!("\nPort not provided. Connecting to largest port number.");
        }
        else {
            println!("\nInput Port");
            println!("==============");
            println!("{}", input_port.to_owned().red());
        }

        print!("\nSearching for port ...");
        io::stdout().flush().unwrap();

        let port_name = match find_port(&input_port) {
            Ok(name) => {
                println!();
                name
            }
            Err(e) => {
                eprintln!("\n{}", e);
                continue;
            }
        };

        let serial_port = match connect_to_port(&port_name) {
            Ok(p) => {
                println!("\n\nConnected!");
                println!("==============\n");
                p
            }
            Err(e) => {
                eprintln!("\n{}\n", e);
                continue;
            }
        };

        input_port = serial_port.name().unwrap();

        if let Err(e) = handle_connection(serial_port) {
            eprintln!("\n\nError: {}", e);
            eprintln!("Disconnected. Retrying Connection...\n");
            continue;
        }
    }
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                            Functions
// —————————————————————————————————————————————————————————————————————————————————————————————————

fn find_port(port_name: &str) -> AnyResult<String> {
    loop {
        let serial_port = serialport::available_ports().context("Failed to list ports")?;

        if !port_name.is_empty() {
            if serial_port.iter().any(|p| p.port_name == port_name) {
                return Ok(port_name.to_string());
            }
        }
        // No port specified
        else {
            // Auto find the port with the longest name or largest number
            if let Some(value) = auto_select_port(serial_port) {
                return Ok(value);
            }
        }

        print!(".");
        io::stdout().flush()?;
        sleep(Duration::from_secs(1));
    }
}

/// Find the port with the longest name or largest number
fn auto_select_port(serial_port: Vec<serialport::SerialPortInfo>) -> Option<String> {
    if serial_port.is_empty() {
        return None;
    }

    let mut sorted_ports = serial_port.clone();
    sorted_ports.sort_by_key(|k| k.port_name.len());

    let name_len = sorted_ports[0].port_name.len();

    if let Some(port) = serial_port
        .iter()
        .filter(|f| f.port_name.len() == name_len)
        .max_by_key(|p| generate_key_from_suffix(&p.port_name))
    {
        return Some(port.port_name.clone());
    }

    None
}

fn generate_key_from_suffix(name: &str) -> u16 {
    if name.is_empty() {
        return 0;
    };

    let mut key = 0_u16;

    if name.ends_with(|pat: char| pat.is_numeric()) {
        name.chars()
            .rev()
            .take_while(|c| c.is_numeric())
            .enumerate()
            .for_each(|f| {
                let i = f.0;
                let n = f.1.to_digit(10).unwrap() as u16;
                key += if i == 0 { n } else { i as u16 * 10 * n };
            });
        return key;
    }
    else {
        return 0;
    }
}

fn connect_to_port(port_name: &str) -> AnyResult<PortType> {
    println!("Connecting to port: {}", port_name.to_owned().red());
    io::stdout().flush()?;

    const ATTEMPTS: u8 = 5;

    for attempt in 0..=ATTEMPTS {
        match serialport::new(port_name, 115_200)
            .dtr_on_open(true)
            .timeout(TIMEOUT)
            .open_native()
        {
            Ok(port) => {
                return Ok(port);
            }
            Err(e) if attempt == ATTEMPTS => {
                return Err(e).context("Failed after 5 attempts");
            }
            Err(e) => {
                println!("Port Error: {}", e.to_string().red());
            }
        }
        sleep(Duration::from_millis(500));
    }
    unreachable!()
}

// ————————————————————————————————————— Handle Serial Data ————————————————————————————————————————

fn handle_connection(serial_port: PortType) -> AnyResult<()> {
    let port_name = serial_port.name().unwrap();

    let (main_thread_tx, main_thread_rx) = mpsc::channel::<ThreadMsg>();
    let (serial_thread_tx, serial_thread_rx) = mpsc::channel::<String>();
    let (data_thread_tx, data_thread_rx) = mpsc::channel::<Data>();

    spawn_serial_thread(serial_port, main_thread_tx.clone(), serial_thread_rx);
    spawn_data_thread(main_thread_tx.clone(), data_thread_rx);

    let mut stdout = std::io::stdout();
    let mut std_output = String::new();
    let mut std_input = String::new();

    'main_rx: loop {
        let msg_result = main_thread_rx.recv_timeout(Duration::from_millis(10));

        if let Ok(msg) = msg_result {
            match msg {
                ThreadMsg::Print(s) => {
                    std_output.push_str(&s);
                }
                ThreadMsg::Error(e) => {
                    eprintln!("Thread Error: {}", e);
                    stdout.write_all(std_output.as_bytes())?;
                    std_output.clear();
                    continue;
                }
                ThreadMsg::Data(data) => {
                    data_thread_tx.send(data).unwrap();
                }
                ThreadMsg::Done => {
                    std_output.push_str("\nThread Done\n");
                }
                ThreadMsg::Started => {
                    std_output.push_str("\nThread Started\n");
                }
                ThreadMsg::Exiting => {
                    std_output.push_str("\nThread Exiting\n");
                    stdout.write_all(std_output.as_bytes())?;
                    std_output.clear();
                    break;
                }
            }
        }

        // ———————————————————————————————————————— Input ——————————————————————————————————————————

        // Read stdin raw - non-blocking
        read_stdin_input(&mut std_input)?;

        // Detect new line in input buffer
        if std_input.ends_with('\n') {
            std_output.push_str(&format!("\n{} {}", ">>:".green(), std_input.clone().blue())); // Print the input line
            serial_thread_tx.send(std_input.clone())?; // Sending to serial thread
            std_input.clear();
        }

        // Write all
        stdout.write_all(std_output.as_bytes())?;
        std_output.clear();

        // —————————————————————————————————————— Input Bar ————————————————————————————————————————

        // Format status msg
        let status_bar_msg = format_args!(
            "{} {} {}",
            port_name.clone().red(),
            ">>:".green(),
            std_input.clone().blue()
        )
        .to_string();

        print_input_bar(&status_bar_msg);
    }
    Ok(())
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                          Data Thread
// —————————————————————————————————————————————————————————————————————————————————————————————————

fn spawn_data_thread(
    main_thread_tx: mpsc::Sender<ThreadMsg>,
    data_thread_rx: mpsc::Receiver<Data>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        'data: loop {
            if let Ok(data) = data_thread_rx.recv() {
                match data.process() {
                    Ok(res) => {
                        main_thread_tx.send(ThreadMsg::Print(res)).unwrap();
                    }
                    Err(e) => {
                        main_thread_tx
                            .send(ThreadMsg::Error(format!("{}", e)))
                            .unwrap();
                    }
                }
            }
        }
    })
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                          Serial Thread
// —————————————————————————————————————————————————————————————————————————————————————————————————

#[derive(Debug)]
pub enum ThreadMsg {
    Started,
    Done,
    Exiting,
    Error(String),
    Print(String),
    Data(Data),
}

fn spawn_serial_thread(
    mut serial_port: PortType,
    main_thread_tx: mpsc::Sender<ThreadMsg>,
    local_thread_rx: mpsc::Receiver<String>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        main_thread_tx.send(ThreadMsg::Started).unwrap();

        let mut buffer = Vec::<u8>::with_capacity(READ_BUFFER_SIZE);
        let mut raw_read = [0u8; READ_BUFFER_SIZE];

        'serial_rw: loop {
            // Serial Write
            if let Ok(output_msg) = local_thread_rx.try_recv() {
                if let Err(e) = serial_port.write(output_msg.as_bytes()) {
                    main_thread_tx
                        .send(ThreadMsg::Error(format!("Serial write error: {:?}", e)))
                        .unwrap();
                    break 'serial_rw;
                };
            }

            // Serial Read
            match serial_port.read(&mut raw_read) {
                Ok(n) => {
                    buffer.extend_from_slice(&raw_read[..n]);

                    // Direct Mode
                    if *DIRECT_MODE.get().unwrap() {
                        main_thread_tx
                            .send(ThreadMsg::Print(format!("{}", String::from_utf8_lossy(&buffer))))
                            .unwrap();
                        buffer.clear();
                        continue 'serial_rw;
                    }

                    // MXS Packet Filtering Mode
                    let MxsFilterResult {
                        skipped_data,
                        trim_index,
                        packets,
                    } = MxsDecoder::filter_buffer(&buffer);

                    // Handle skipped non-packet slice
                    if !skipped_data.is_empty() {
                        main_thread_tx
                            .send(ThreadMsg::Print(format!(
                                "{}",
                                String::from_utf8_lossy(skipped_data)
                            )))
                            .unwrap();
                    }

                    // ---- Process Packets based on type
                    if !packets.is_empty() {
                        for packet in &packets {
                            match &packet.packet_type {
                                // Sized Data
                                MxsPacketType::Data => {
                                    let packet_data = packet.data;

                                    if let Ok(data) = Data::try_from(packet_data) {
                                        main_thread_tx.send(ThreadMsg::Data(data)).unwrap();
                                    }
                                    else {
                                        main_thread_tx
                                            .send(ThreadMsg::Error(
                                                "Couldn't convert byte stream into data".into(),
                                            ))
                                            .unwrap();
                                    }
                                }
                                // Unsized Msg Packets
                                MxsPacketType::End => {
                                    main_thread_tx
                                        .send(ThreadMsg::Print("Received: End\n".into()))
                                        .unwrap();
                                }

                                // Other Notification Packets
                                p => {
                                    main_thread_tx
                                        .send(ThreadMsg::Print(format!("Received: {:?}\n", p)))
                                        .unwrap();
                                }
                            }
                        }
                    } // ----

                    // Remove processed slice
                    buffer.drain(..trim_index);
                }

                // Timeout > Ignore
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),

                // Error > Return
                Err(ref e) => {
                    main_thread_tx
                        .send(ThreadMsg::Error(format!("Serial read error: {:?}", e)))
                        .unwrap();
                    break 'serial_rw;
                }
            };
        }

        // Done
        main_thread_tx.send(ThreadMsg::Exiting).unwrap();
    })
}
