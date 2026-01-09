use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

// --- Configuration Constants ---
const TCP_PORT: u16 = 8080;
const UDP_PORT: u16 = 7070;
const BUFFER_SIZE: usize = 1024 * 4;
const UDP_PAYLOAD_SIZE: usize = 1024;
const TEST_DURATION: Duration = Duration::from_secs(5);

// --- Protocol Constants ---
const CMD_DOWNLOAD: u8 = 0x01;
const CMD_UPLOAD: u8 = 0x02;
const CMD_ACK: u8 = 0x03;
const CMD_DONE: u8 = 0x04;

fn main() {
    println!("=== Starting Speed Test Server ===");
    println!("TCP Listening on port {}", TCP_PORT);
    println!("UDP Listening on port {}", UDP_PORT);
    println!("Protocol: DOWNLOAD=0x01, UPLOAD=0x02, ACK=0x03, DONE=0x04");
    println!();

    let tcp_handle = thread::spawn(|| {
        start_tcp_listener();
    });

    let udp_handle = thread::spawn(|| {
        start_udp_listener();
    });

    let _ = tcp_handle.join();
    let _ = udp_handle.join();
}

// ==========================================
//              TCP HANDLING
// ==========================================

fn start_tcp_listener() {
    let listener =
        TcpListener::bind(format!("0.0.0.0:{}", TCP_PORT)).expect("Failed to bind TCP port");

    println!("[TCP] Listener started successfully");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!(
                    "\n[TCP] ===== New Client Connected: {:?} =====",
                    stream.peer_addr()
                );
                thread::spawn(|| {
                    handle_tcp_client(stream);
                });
            }
            Err(e) => {
                eprintln!("[TCP] Connection failed: {}", e);
            }
        }
    }
}

fn handle_tcp_client(mut stream: TcpStream) {
    // Disable Nagle's algorithm to send data immediately
    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("[TCP] Failed to set nodelay: {}", e);
        return;
    }
    println!("[TCP] Set TCP_NODELAY = true");

    // Use longer timeouts to prevent premature disconnection
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    let mut command_buf = [0u8; 1];

    loop {
        println!("[TCP] Waiting for command...");
        match stream.read_exact(&mut command_buf) {
            Ok(_) => {
                let cmd = command_buf[0];
                println!("[TCP] Received command: 0x{:02x}", cmd);

                match cmd {
                    CMD_DOWNLOAD => {
                        println!("[TCP] >>> Starting DOWNLOAD Test <<<");
                        if !handle_tcp_download(&mut stream) {
                            println!("[TCP] Download test failed, closing connection");
                            break;
                        }
                        println!("[TCP] >>> DOWNLOAD Test Complete <<<");
                    }
                    CMD_UPLOAD => {
                        println!("[TCP] >>> Starting UPLOAD Test <<<");
                        if !handle_tcp_upload(&mut stream) {
                            println!("[TCP] Upload test failed, closing connection");
                            break;
                        }
                        println!("[TCP] >>> UPLOAD Test Complete <<<");
                    }
                    _ => {
                        eprintln!("[TCP] ERROR: Unknown command 0x{:02x}", cmd);
                        break;
                    }
                }
            }
            Err(e) => {
                println!("[TCP] Client disconnected: {}", e);
                break;
            }
        }
    }
    println!("[TCP] ===== Client Session Ended =====\n");
}

fn handle_tcp_download(stream: &mut TcpStream) -> bool {
    println!("[TCP-DL] Step 1: Sending ACK (0x{:02x})", CMD_ACK);

    // Send ACK
    if let Err(e) = stream.write_all(&[CMD_ACK]) {
        eprintln!("[TCP-DL] ERROR: Failed to write ACK: {}", e);
        return false;
    }
    if let Err(e) = stream.flush() {
        eprintln!("[TCP-DL] ERROR: Failed to flush ACK: {}", e);
        return false;
    }
    println!("[TCP-DL] Step 2: ACK sent and flushed");

    // Small delay to ensure ACK is received before data flood
    thread::sleep(Duration::from_millis(10));

    println!(
        "[TCP-DL] Step 3: Starting data transmission for {} seconds",
        TEST_DURATION.as_secs()
    );

    let data = [1u8; BUFFER_SIZE];
    let start_time = Instant::now();
    let mut bytes_sent = 0u64;
    let mut packets_sent = 0u64;

    while start_time.elapsed() < TEST_DURATION {
        match stream.write(&data) {
            Ok(n) => {
                bytes_sent += n as u64;
                packets_sent += 1;
            }
            Err(e) => {
                eprintln!(
                    "[TCP-DL] ERROR: Write error after {} bytes: {}",
                    bytes_sent, e
                );
                return false;
            }
        }
    }

    if let Err(e) = stream.flush() {
        eprintln!("[TCP-DL] ERROR: Final flush failed: {}", e);
        return false;
    }

    println!("[TCP-DL] Step 4: Data transmission complete");
    println!("[TCP-DL]   - Sent {} packets", packets_sent);
    println!(
        "[TCP-DL]   - Sent {} bytes ({:.2} MB)",
        bytes_sent,
        bytes_sent as f64 / 1_000_000.0
    );

    // Small delay before sending DONE
    thread::sleep(Duration::from_millis(10));

    println!("[TCP-DL] Step 5: Sending DONE signal (0x{:02x})", CMD_DONE);
    if let Err(e) = stream.write_all(&[CMD_DONE]) {
        eprintln!("[TCP-DL] ERROR: Failed to send DONE: {}", e);
        return false;
    }
    if let Err(e) = stream.flush() {
        eprintln!("[TCP-DL] ERROR: Failed to flush DONE: {}", e);
        return false;
    }

    println!("[TCP-DL] Step 6: DONE signal sent");
    true
}

fn handle_tcp_upload(stream: &mut TcpStream) -> bool {
    println!("[TCP-UL] Step 1: Sending ACK (0x{:02x})", CMD_ACK);

    if let Err(e) = stream.write_all(&[CMD_ACK]) {
        eprintln!("[TCP-UL] ERROR: Failed to send ACK: {}", e);
        return false;
    }
    if let Err(e) = stream.flush() {
        eprintln!("[TCP-UL] ERROR: Failed to flush ACK: {}", e);
        return false;
    }
    println!("[TCP-UL] Step 2: ACK sent, ready to receive");

    let mut buffer = [0u8; BUFFER_SIZE];
    let start_time = Instant::now();
    let mut bytes_received = 0u64;
    let mut reads = 0u64;

    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

    println!(
        "[TCP-UL] Step 3: Receiving data for {} seconds",
        TEST_DURATION.as_secs()
    );

    while start_time.elapsed() < TEST_DURATION {
        match stream.read(&mut buffer) {
            Ok(0) => {
                println!("[TCP-UL] WARNING: Client closed connection early");
                return false;
            }
            Ok(n) => {
                bytes_received += n as u64;
                reads += 1;
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                {
                    continue;
                } else {
                    eprintln!("[TCP-UL] ERROR: Read error: {}", e);
                    return false;
                }
            }
        }
    }

    println!("[TCP-UL] Step 4: Reception complete");
    println!("[TCP-UL]   - Received {} reads", reads);
    println!(
        "[TCP-UL]   - Received {} bytes ({:.2} MB)",
        bytes_received,
        bytes_received as f64 / 1_000_000.0
    );

    thread::sleep(Duration::from_millis(10));

    println!("[TCP-UL] Step 5: Sending DONE signal (0x{:02x})", CMD_DONE);
    if let Err(e) = stream.write_all(&[CMD_DONE]) {
        eprintln!("[TCP-UL] ERROR: Failed to send DONE: {}", e);
        return false;
    }
    if let Err(e) = stream.flush() {
        eprintln!("[TCP-UL] ERROR: Failed to flush DONE: {}", e);
        return false;
    }

    println!("[TCP-UL] Step 6: DONE signal sent");
    true
}

// ==========================================
//              UDP HANDLING
// ==========================================

fn start_udp_listener() {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_PORT)).expect("Failed to bind UDP port");
    println!("[UDP] Listener started successfully");

    let mut buf = [0u8; 1024];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, src_addr)) => {
                if size > 0 {
                    let command = buf[0];
                    println!(
                        "\n[UDP] Received command 0x{:02x} from {}",
                        command, src_addr
                    );

                    match command {
                        CMD_DOWNLOAD => {
                            println!("[UDP] >>> Starting DOWNLOAD Test for {} <<<", src_addr);

                            // Send ACK immediately
                            println!("[UDP-DL] Step 1: Sending ACK (0x{:02x})", CMD_ACK);
                            if let Err(e) = socket.send_to(&[CMD_ACK], src_addr) {
                                eprintln!("[UDP-DL] ERROR: Failed to send ACK: {}", e);
                                continue;
                            }
                            println!("[UDP-DL] Step 2: ACK sent");

                            let socket_clone = socket.try_clone().expect("Failed to clone socket");
                            thread::spawn(move || {
                                handle_udp_download(socket_clone, src_addr);
                            });
                        }
                        CMD_UPLOAD => {
                            // This is the upload test - server just sinks the data
                            // No response needed, just acknowledge receipt
                        }
                        _ => {
                            eprintln!("[UDP] ERROR: Unknown command: 0x{:02x}", command);
                        }
                    }
                }
            }
            Err(e) => eprintln!("[UDP] Receive error: {}", e),
        }
    }
}

fn handle_udp_download(socket: UdpSocket, addr: std::net::SocketAddr) {
    println!("[UDP-DL] Step 3: Starting packet transmission");

    let payload = [1u8; UDP_PAYLOAD_SIZE];
    let start_time = Instant::now();
    let mut packet_count = 0u64;
    let mut errors = 0u64;

    while start_time.elapsed() < TEST_DURATION {
        match socket.send_to(&payload, addr) {
            Ok(_) => packet_count += 1,
            Err(e) => {
                errors += 1;
                if errors == 1 {
                    eprintln!("[UDP-DL] ERROR: Send error: {}", e);
                }
                break;
            }
        }

        // Small delay to prevent overwhelming the network
        thread::sleep(Duration::from_micros(10));
    }

    println!("[UDP-DL] Step 4: Transmission complete");
    println!("[UDP-DL]   - Sent {} packets", packet_count);
    println!(
        "[UDP-DL]   - Total bytes: {} ({:.2} MB)",
        packet_count * UDP_PAYLOAD_SIZE as u64,
        (packet_count * UDP_PAYLOAD_SIZE as u64) as f64 / 1_000_000.0
    );

    // Send completion signal
    println!("[UDP-DL] Step 5: Sending DONE signal (0x{:02x})", CMD_DONE);
    if let Err(e) = socket.send_to(&[CMD_DONE], addr) {
        eprintln!("[UDP-DL] ERROR: Failed to send DONE signal: {}", e);
    } else {
        println!("[UDP-DL] Step 6: DONE signal sent");
    }

    println!("[UDP] >>> DOWNLOAD Test Complete for {} <<<\n", addr);
}
