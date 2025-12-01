mod mxs_decoder;
mod mxs_shared;

use mxs_decoder::*;

use std::sync::mpsc;
use std::{env, io};

use std::io::{Read, Write};
use std::thread::{self, JoinHandle, sleep};
use std::time::Duration;

use anyhow::{Context, Result};

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                             Globals
// —————————————————————————————————————————————————————————————————————————————————————————————————

const TIMEOUT: Duration = Duration::from_millis(500);
const READ_BUFFER_SIZE: usize = 2000;

#[cfg(unix)]
type PortType = serialport::TTYPort;
#[cfg(windows)]
type PortType = serialport::COMPort;

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                              Main
// —————————————————————————————————————————————————————————————————————————————————————————————————

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("\n=== Serial Link Started ===");
    println!("     with MXS Protocol \n");

    loop {
        let input_port = args.get(1).map(|s| s.as_str()).unwrap_or("");

        if input_port.is_empty() {
            println!("\nPort not provided. Connecting to largest port number.");
        }
        else {
            println!("\nInput Port");
            println!("==============");
            println!("{input_port}");
        }

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

        let port = match connect_to_port(&port_name) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("\n{}\n", e);
                continue;
            }
        };

        if let Err(e) = handle_serial_read(port) {
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

fn find_port(input_port: &str) -> Result<String> {
    loop {
        let ports = serialport::available_ports().context("Failed to list ports")?;

        if !input_port.is_empty() {
            if ports.iter().any(|p| p.port_name == input_port) {
                return Ok(input_port.to_string());
            }
        }
        else {
            // Get highest port
            if let Some(port) = ports
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

fn connect_to_port(port_name: &str) -> Result<PortType> {
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

fn handle_serial_read(port: PortType) -> Result<()> {
    let (main_tx, main_rx) = mpsc::channel::<ThreadMsg>();

    spawn_reader_thread(port, main_tx.clone());

    loop {
        let msg = main_rx.recv()?;
        match msg {
            ThreadMsg::Print(s) => {
                print!("{s}");
                continue;
            }
            ThreadMsg::Error(e) => {
                eprintln!("Thread Error: {}", e);
                continue;
            }
            ThreadMsg::Data(data) => {
                process_data(data)?;
            }
            ThreadMsg::Done => {
                println!("Thread Done");
                continue;
            }
            ThreadMsg::Started => {
                println!("\nThread Started");
                continue;
            }
            ThreadMsg::Exiting => {
                println!("\nThread Exiting");
                break;
            }
        }
    }
    Ok(())
}

// —————————————————————————————————————————————————————————————————————————————————————————————————
//                                          Reader Thread
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

fn spawn_reader_thread(mut port: PortType, tx: mpsc::Sender<ThreadMsg>) -> JoinHandle<()> {
    thread::spawn(move || {
        //
        tx.send(ThreadMsg::Started).unwrap();

        let mut buffer = Vec::<u8>::with_capacity(READ_BUFFER_SIZE);
        let mut raw_read = [0u8; READ_BUFFER_SIZE];

        'read: loop {
            match port.read(&mut raw_read) {
                Ok(n) => {
                    buffer.extend_from_slice(&raw_read[..n]);

                    let MxsFilterResult {
                        skipped_data,
                        trim_index,
                        packets,
                    } = MxsDecoder::filter_buffer(&buffer);

                    // Handle skipped non-packet slice
                    if !skipped_data.is_empty() {
                        tx.send(ThreadMsg::Print(format!(
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
                                        tx.send(ThreadMsg::Data(data)).unwrap();
                                    }
                                    else {
                                        tx.send(ThreadMsg::Error(
                                            "Couldn't convert byte stream into data".into(),
                                        ))
                                        .unwrap();
                                    }
                                }
                                // Unsized Msg Packets
                                MxsPacketType::End => {
                                    tx.send(ThreadMsg::Print("Received: End\n".into())).unwrap();
                                }

                                p => {
                                    tx.send(ThreadMsg::Print(format!("Received: {:?}\n", p)))
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
                    let _ = tx.send(ThreadMsg::Error(format!("Serial read error: {:?}", e)));
                    break 'read;
                }
            };
        }

        // Done
        tx.send(ThreadMsg::Exiting).unwrap();
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

pub fn process_data(data: Data) -> Result<()> {
    // TODO: do something with data
    println!("Thread Data: {:?}", data);
    Ok(())
}
