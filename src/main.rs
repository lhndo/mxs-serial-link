mod mxs_decoder;
mod mxs_shared;
mod stdio_helper;

use std::env;
use std::io::Read;
use std::sync::{OnceLock, mpsc};
use std::thread::{self, JoinHandle, sleep};

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

#[cfg(unix)]
type PortType = serialport::TTYPort;
#[cfg(windows)]
type PortType = serialport::COMPort;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                              Main
// —————————————————————————————————————————————————————————————————————————————————————————————————

fn main() {
    // ——————————————————————————————————————— Stdio Init ——————————————————————————————————————————

    init_process!();

    // —————————————————————————————————————————— Args —————————————————————————————————————————————

    let args: Vec<String> = env::args().collect();
    println!("\n=== Serial Link Started ===");

    // Direct mode skips MXS packet filtering
    let direct = if args.contains(&"--direct".to_string()) {
        println!("        Direct mode \n");
        true
    }
    else {
        println!("     with MXS Protocol \n");
        false
    };

    DIRECT_MODE.set(direct).ok();

    // First argument should be the port name
    let input_port = args
        .get(1)
        .map(|s| if s != "--direct" { s } else { "" })
        .unwrap_or("");

    if input_port.is_empty() {
        println!("\nPort not provided. Connecting to largest port number.");
    }
    else {
        println!("\nInput Port");
        println!("==============");
        println!("{input_port}");
    }

    // ———————————————————————————————————————— Main Loop ——————————————————————————————————————————

    'main: loop {
        println!("\nAvailable Ports");
        println!("==============");
        if let Ok(ports) = serialport::available_ports() {
            for port in &ports {
                println!("{}", port.port_name);
            }
        }
        else {
            println!("No Ports")
        }
        println!("______________");

        print!("\nSearching for port ...");
        io::stdout().flush().unwrap();

        let port_name = match find_port(input_port) {
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
            Ok(p) => p,
            Err(e) => {
                eprintln!("\n{}\n", e);
                continue;
            }
        };

        if let Err(e) = handle_serial_port(serial_port) {
            eprintln!("\n\nError: {}", e);
            eprintln!("Disconnected. Retrying Connection...\n");
            continue;
        }
    }
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                            Functions
// —————————————————————————————————————————————————————————————————————————————————————————————————

// ———————————————————————————————————————————— Ports ——————————————————————————————————————————————

fn find_port(port_name: &str) -> AnyResult<String> {
    loop {
        let serial_port = serialport::available_ports().context("Failed to list ports")?;

        if !port_name.is_empty() {
            if serial_port.iter().any(|p| p.port_name == port_name) {
                return Ok(port_name.to_string());
            }
        }
        else {
            // Get highest port
            if let Some(port) = serial_port
                .iter()
                .max_by_key(|p| p.port_name.char_indices().last().unwrap_or((0, '0')).1)
            {
                return Ok(port.port_name.clone());
            }
        }

        print!(".");
        io::stdout().flush()?;
        sleep(Duration::from_secs(1));
    }
}

fn connect_to_port(port_name: &str) -> AnyResult<PortType> {
    print!("Connecting to port: {port_name}");
    io::stdout().flush()?;

    for attempt in 0..10 {
        match serialport::new(port_name, 115_200)
            .dtr_on_open(true)
            .timeout(TIMEOUT)
            .open_native()
        {
            Ok(port) => {
                println!("\n\nConnected!");
                println!("==============\n");
                return Ok(port);
            }
            Err(e) if attempt == 9 => {
                return Err(e).context("Failed after 10 attempts");
            }
            _ => {
                print!(".");
                io::stdout().flush()?;
                sleep(Duration::from_millis(500));
            }
        }
    }
    unreachable!()
}

// ————————————————————————————————————— Handle Serial Data ————————————————————————————————————————

fn handle_serial_port(serial_port: PortType) -> AnyResult<()> {
    let (main_tx, main_rx) = mpsc::channel::<ThreadMsg>();
    let (thread_tx, thread_rx) = mpsc::channel::<String>();
    let port_name = serial_port.name().unwrap();

    spawn_serial_thread(serial_port, main_tx.clone(), thread_rx);

    let mut stdout = std::io::stdout();
    let mut std_output = String::new();
    let mut std_input = String::new();
    let mut std_input_history = VecDeque::<String>::new();

    loop {
        // let msg = main_rx.recv()?;
        if let Ok(msg) = main_rx.try_recv() {
            match msg {
                ThreadMsg::Print(s) => {
                    std_output.push_str(&s);
                }
                ThreadMsg::Error(e) => {
                    eprintln!("Thread Error: {}", e);
                    continue;
                }
                ThreadMsg::Data(data) => {
                    // process_data(data)?;
                }
                ThreadMsg::Done => {
                    std_output.push_str(&"\nThread Done\n");
                }
                ThreadMsg::Started => {
                    std_output.push_str(&"\nThread Started\n");
                }
                ThreadMsg::Exiting => {
                    std_output.push_str(&"\nThread Exiting\n");
                    break;
                }
            }
        }

        // Read stdin raw - non-blocking
        stdin_read_raw(&mut std_input, &mut std_input_history)?;

        // Detect new line in input buffer
        if std_input.ends_with('\n') {
            std_output.push_str(&format!("\n{} {}", ">>:".green(), std_input.clone().blue()));
            thread_tx.send(std_input.clone())?; // Sending to serial thread
            std_input.clear();
        }

        // Output buffer
        stdout.write(std_output.as_bytes())?;
        std_output.clear();

        // Format status msg
        let status_bar_msg = format_args!(
            "{} {} {}",
            port_name.clone().red(),
            ">>:".green(),
            std_input.clone().blue()
        )
        .to_string();

        print_status_bar(&status_bar_msg);

        // Avoiding a tight loop
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
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
    main_tx: mpsc::Sender<ThreadMsg>,
    thread_rx: mpsc::Receiver<String>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        main_tx.send(ThreadMsg::Started).unwrap();

        let mut buffer = Vec::<u8>::with_capacity(READ_BUFFER_SIZE);
        let mut raw_read = [0u8; READ_BUFFER_SIZE];

        'serial_rw: loop {
            // Serial Write
            if let Ok(output_msg) = thread_rx.try_recv() {
                if let Err(e) = serial_port.write(output_msg.as_bytes()) {
                    main_tx
                        .send(ThreadMsg::Error(format!("Serial write error: {:?}", e)))
                        .unwrap();
                    break 'serial_rw;
                };
            }

            // Serial Read
            match serial_port.read(&mut raw_read) {
                Ok(n) => {
                    buffer.extend_from_slice(&raw_read[..n]);

                    if *DIRECT_MODE.get().unwrap() {
                        main_tx
                            .send(ThreadMsg::Print(format!("{}", String::from_utf8_lossy(&buffer))))
                            .unwrap();
                        buffer.clear();
                        continue 'serial_rw;
                    }

                    let MxsFilterResult {
                        skipped_data,
                        trim_index,
                        packets,
                    } = MxsDecoder::filter_buffer(&buffer);

                    // Handle skipped non-packet slice
                    if !skipped_data.is_empty() {
                        main_tx
                            .send(ThreadMsg::Print(format!(
                                "{}",
                                String::from_utf8_lossy(&skipped_data)
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

                                    if let Some(data) = Data::try_from(packet_data) {
                                        main_tx.send(ThreadMsg::Data(data)).unwrap();
                                    }
                                    else {
                                        main_tx
                                            .send(ThreadMsg::Error(
                                                "Couldn't convert byte stream into data".into(),
                                            ))
                                            .unwrap();
                                    }
                                }
                                // Unsized Msg Packets
                                MxsPacketType::End => {
                                    main_tx
                                        .send(ThreadMsg::Print("Received: End\n".into()))
                                        .unwrap();
                                }

                                p => {
                                    main_tx
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
                    main_tx
                        .send(ThreadMsg::Error(format!("Serial read error: {:?}", e)))
                        .unwrap();
                    break 'serial_rw;
                }
            };
        }

        // Done
        main_tx.send(ThreadMsg::Exiting).unwrap();
    })
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                              Data
// —————————————————————————————————————————————————————————————————————————————————————————————————

#[derive(Debug, Default, Clone, Copy)]
pub struct Data(i16, i16, i16);

impl Data {
    pub fn try_from(buf: &[u8]) -> Option<Self> {
        if buf.len() != size_of::<Self>() {
            return None;
        }

        let data = Self(
            i16::from_le_bytes(buf[0..2].try_into().ok()?),
            i16::from_le_bytes(buf[2..4].try_into().ok()?),
            i16::from_le_bytes(buf[4..6].try_into().ok()?),
        );

        Some(data)
    }
}

// ———————————————————————————————————————— Process Data ———————————————————————————————————————————

pub fn process_data(data: Data) -> AnyResult<()> {
    // TODO: do something with data
    println!("Thread Data: {:?}", data);
    Ok(())
}
