//! Speed test server.
//!
//! Listens on a TCP port and a UDP port and runs throughput tests on request.
//! A client sends a one-byte command (download or upload); the server replies
//! with ACK, streams or sinks data for `TEST_DURATION`, and then sends DONE.

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

// --- Configuration Constants ---
const TCP_PORT: u16 = 8080;
const UDP_PORT: u16 = 7070;
/// Size of each chunk written to / read from a TCP stream.
const BUFFER_SIZE: usize = 1024 * 4;
/// Size of each UDP datagram sent during a download test.
const UDP_PAYLOAD_SIZE: usize = 1024;
/// How long each download/upload test runs.
const TEST_DURATION: Duration = Duration::from_secs(5);

// --- Protocol Constants ---
// Must match the values used by the GUI client.
const CMD_DOWNLOAD: u8 = 0x01;
const CMD_UPLOAD: u8 = 0x02;
const CMD_ACK: u8 = 0x03;
const CMD_DONE: u8 = 0x04;

fn main() {
    println!("=== Starting Speed Test Server ===");
    println!("TCP Listening on port {}", TCP_PORT);
    println!("UDP Listening on port {}", UDP_PORT);

    // Run the TCP and UDP listeners side by side; neither returns in normal operation.
    let tcp_handle = thread::spawn(start_tcp_listener);
    let udp_handle = thread::spawn(start_udp_listener);

    let _ = tcp_handle.join();
    let _ = udp_handle.join();
}

// ==========================================
//              TCP HANDLING
// ==========================================

/// Accepts TCP connections and handles each client on its own thread.
fn start_tcp_listener() {
    let listener =
        TcpListener::bind(format!("0.0.0.0:{}", TCP_PORT)).expect("Failed to bind TCP port");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || handle_tcp_client(stream));
            }
            Err(e) => eprintln!("[TCP] Connection failed: {}", e),
        }
    }
}

/// Reads commands from a connected client until it disconnects or a test fails.
fn handle_tcp_client(mut stream: TcpStream) {
    // Disable Nagle's algorithm so the single-byte ACK/DONE signals go out immediately.
    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("[TCP] Failed to set nodelay: {}", e);
        return;
    }

    // Generous timeouts so slow links aren't dropped mid-test.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    let mut command_buf = [0u8; 1];

    // Any read error here (including a clean disconnect) ends the session.
    while stream.read_exact(&mut command_buf).is_ok() {
        let ok = match command_buf[0] {
            CMD_DOWNLOAD => handle_tcp_download(&mut stream),
            CMD_UPLOAD => handle_tcp_upload(&mut stream),
            cmd => {
                eprintln!("[TCP] Unknown command 0x{:02x}", cmd);
                false
            }
        };
        if !ok {
            break;
        }
    }
}

/// Writes a single protocol byte and flushes it.
fn send_signal(stream: &mut TcpStream, signal: u8) -> std::io::Result<()> {
    stream.write_all(&[signal])?;
    stream.flush()
}

/// Download test: ACK, flood the client with data for `TEST_DURATION`, then DONE.
/// Returns `false` if the connection should be closed.
fn handle_tcp_download(stream: &mut TcpStream) -> bool {
    if let Err(e) = send_signal(stream, CMD_ACK) {
        eprintln!("[TCP-DL] Failed to send ACK: {}", e);
        return false;
    }

    // Small delay so the ACK arrives on its own, ahead of the data flood.
    thread::sleep(Duration::from_millis(10));

    let data = [1u8; BUFFER_SIZE];
    let start_time = Instant::now();

    while start_time.elapsed() < TEST_DURATION {
        if let Err(e) = stream.write(&data) {
            eprintln!("[TCP-DL] Write error: {}", e);
            return false;
        }
    }

    if let Err(e) = stream.flush() {
        eprintln!("[TCP-DL] Final flush failed: {}", e);
        return false;
    }

    // Small delay so DONE is less likely to be coalesced with the last data chunk.
    thread::sleep(Duration::from_millis(10));

    if let Err(e) = send_signal(stream, CMD_DONE) {
        eprintln!("[TCP-DL] Failed to send DONE: {}", e);
        return false;
    }
    true
}

/// Upload test: ACK, read and discard client data for `TEST_DURATION`, then DONE.
/// Returns `false` if the connection should be closed.
fn handle_tcp_upload(stream: &mut TcpStream) -> bool {
    if let Err(e) = send_signal(stream, CMD_ACK) {
        eprintln!("[TCP-UL] Failed to send ACK: {}", e);
        return false;
    }

    let mut buffer = [0u8; BUFFER_SIZE];
    let start_time = Instant::now();

    // Short timeout so a stalled client doesn't keep us past the test window.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

    while start_time.elapsed() < TEST_DURATION {
        match stream.read(&mut buffer) {
            // Client closed the connection before the test finished.
            Ok(0) => return false,
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => {
                eprintln!("[TCP-UL] Read error: {}", e);
                return false;
            }
        }
    }

    thread::sleep(Duration::from_millis(10));

    if let Err(e) = send_signal(stream, CMD_DONE) {
        eprintln!("[TCP-UL] Failed to send DONE: {}", e);
        return false;
    }
    true
}

// ==========================================
//              UDP HANDLING
// ==========================================

/// Receives UDP commands. Download requests are served on a separate thread;
/// upload datagrams are simply discarded.
fn start_udp_listener() {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", UDP_PORT)).expect("Failed to bind UDP port");

    let mut buf = [0u8; 1024];

    loop {
        let (size, src_addr) = match socket.recv_from(&mut buf) {
            Ok(received) => received,
            Err(e) => {
                eprintln!("[UDP] Receive error: {}", e);
                continue;
            }
        };
        // Commands are single-byte datagrams. Anything larger is upload payload,
        // which the server just discards. (Payload bytes are 0x01, the same value
        // as CMD_DOWNLOAD, so they must not be read as commands.)
        if size != 1 {
            continue;
        }

        match buf[0] {
            CMD_DOWNLOAD => {
                if let Err(e) = socket.send_to(&[CMD_ACK], src_addr) {
                    eprintln!("[UDP-DL] Failed to send ACK: {}", e);
                    continue;
                }

                // Stream on a cloned socket so this loop keeps receiving.
                let socket_clone = socket.try_clone().expect("Failed to clone socket");
                thread::spawn(move || handle_udp_download(socket_clone, src_addr));
            }
            // The upload trigger needs no reply; the client just starts sending.
            CMD_UPLOAD => {}
            cmd => eprintln!("[UDP] Unknown command 0x{:02x}", cmd),
        }
    }
}

/// Sends datagrams to `addr` for `TEST_DURATION`, then a DONE signal.
fn handle_udp_download(socket: UdpSocket, addr: SocketAddr) {
    let payload = [1u8; UDP_PAYLOAD_SIZE];
    let start_time = Instant::now();

    while start_time.elapsed() < TEST_DURATION {
        if let Err(e) = socket.send_to(&payload, addr) {
            eprintln!("[UDP-DL] Send error: {}", e);
            break;
        }

        // Brief pause so we don't overrun the send buffer.
        thread::sleep(Duration::from_micros(10));
    }

    if let Err(e) = socket.send_to(&[CMD_DONE], addr) {
        eprintln!("[UDP-DL] Failed to send DONE signal: {}", e);
    }
}
