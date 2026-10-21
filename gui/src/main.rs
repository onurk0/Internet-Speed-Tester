//! Speed test GUI client.
//!
//! An `iced` desktop app that connects to the speed test server over TCP or
//! UDP, runs a download test followed by an upload test, and shows live
//! throughput in Mbps. The network work runs on a background thread that
//! reports progress back to the UI over an `mpsc` channel.

use iced::{
    alignment, time,
    widget::{button, column, container, image, row, text, text_input, toggler, Stack},
    Background, Border, Color, Element, Length, Size, Subscription, Task, Theme,
};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub fn main() -> iced::Result {
    iced::application(
        "Speed Testing",
        NetworkConfigApp::update,
        NetworkConfigApp::view,
    )
    .window_size(Size::new(800.0, 600.0))
    .theme(|_| Theme::Dark)
    .subscription(NetworkConfigApp::subscription)
    .run_with(NetworkConfigApp::new)
}

// --- Protocol Constants ---
// Must match the values used by the server.
const CMD_DOWNLOAD: u8 = 0x01;
const CMD_UPLOAD: u8 = 0x02;
const CMD_ACK: u8 = 0x03;
const CMD_DONE: u8 = 0x04;

// --- Default Ports ---
const DEFAULT_TCP_PORT: &str = "8080";
const DEFAULT_UDP_PORT: &str = "7070";

/// How often live speed updates are sent to the UI during a test.
const REPORT_INTERVAL: Duration = Duration::from_millis(500);
/// How long the client sends data during an upload test.
const UPLOAD_DURATION: Duration = Duration::from_secs(5);

// --- MESSAGES ---
#[derive(Debug, Clone)]
pub enum Message {
    IpAddressChanged(String),
    PortChanged(String),
    ProtocolToggled(bool),
    /// "Connect" button pressed: start a test run.
    Connect,
    /// Periodic tick while a test is running; drains the worker's progress channel.
    Poll,
}

/// A progress report sent from the worker thread to the UI.
#[derive(Debug, Clone)]
pub enum TestProgress {
    /// A status update, optionally with a new speed reading.
    Update {
        phase: String,
        mbps: f64,
        is_download: bool,
    },
    /// The run is over (successfully or not); `phase` is the final status text.
    Finished { phase: String },
}

// --- APP STATE ---
#[derive(Debug)]
pub struct NetworkConfigApp {
    ip_address: String,
    port: String,
    is_tcp: bool,
    background_image: iced::widget::image::Handle,
    is_testing: bool,
    current_phase: String,
    download_speed: f64,
    upload_speed: f64,
    /// Receives progress from the worker thread while a test is running.
    receiver: Option<mpsc::Receiver<TestProgress>>,
}

impl NetworkConfigApp {
    fn new() -> (Self, Task<Message>) {
        // Path is relative to the working directory, so run from `gui/`.
        let background_image = image::Handle::from_path("TCNJ_Speed.jpg");

        (
            Self {
                ip_address: String::from("127.0.0.1"),
                port: String::from(DEFAULT_TCP_PORT),
                is_tcp: true,
                background_image,
                is_testing: false,
                current_phase: String::from("Idle"),
                download_speed: 0.0,
                upload_speed: 0.0,
                receiver: None,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IpAddressChanged(val) => self.ip_address = val,
            Message::PortChanged(val) => self.port = val,
            Message::ProtocolToggled(tcp) => {
                // Switch to the server's port for the chosen protocol.
                self.is_tcp = tcp;
                let port = if tcp { DEFAULT_TCP_PORT } else { DEFAULT_UDP_PORT };
                self.port = port.into();
            }
            Message::Connect => {
                // Ignore clicks while a test is already running.
                if self.is_testing {
                    return Task::none();
                }

                self.is_testing = true;
                self.download_speed = 0.0;
                self.upload_speed = 0.0;
                self.current_phase = "Starting...".into();

                let (tx, rx) = mpsc::channel();
                self.receiver = Some(rx);

                let addr = format!("{}:{}", self.ip_address, self.port);
                let tcp = self.is_tcp;

                // Run the blocking network test off the UI thread.
                thread::spawn(move || {
                    if tcp {
                        run_tcp_test(&addr, &tx);
                    } else {
                        run_udp_test(&addr, &tx);
                    }
                });
            }
            Message::Poll => self.drain_progress(),
        }
        Task::none()
    }

    /// Applies every progress report the worker has sent since the last poll.
    fn drain_progress(&mut self) {
        let Some(rx) = &self.receiver else {
            return;
        };

        let mut finished = false;
        for progress in rx.try_iter() {
            match progress {
                TestProgress::Update {
                    phase,
                    mbps,
                    is_download,
                } => {
                    self.current_phase = phase;
                    if is_download {
                        self.download_speed = mbps;
                    } else {
                        self.upload_speed = mbps;
                    }
                }
                TestProgress::Finished { phase } => {
                    self.current_phase = phase;
                    finished = true;
                }
            }
        }

        if finished {
            self.is_testing = false;
            self.receiver = None;
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        // Only poll the worker channel while a test is in progress.
        if self.is_testing {
            time::every(Duration::from_millis(100)).map(|_| Message::Poll)
        } else {
            Subscription::none()
        }
    }

    fn view(&self) -> Element<Message> {
        let background = image(self.background_image.clone())
            .width(Length::Fill)
            .height(Length::Fill);

        let content = container(
            column![
                text("Speed Testing").size(32).color(Color::WHITE),
                // Live status and results panel
                container(
                    column![
                        text(format!("Status: {}", self.current_phase))
                            .size(18)
                            .color(Color::from_rgb(1.0, 0.8, 0.0)),
                        row![
                            text(format!("Download: {:.2} Mbps", self.download_speed))
                                .size(20)
                                .color(Color::from_rgb(0.0, 1.0, 0.0)),
                            text("|").size(20).color(Color::WHITE),
                            text(format!("Upload: {:.2} Mbps", self.upload_speed))
                                .size(20)
                                .color(Color::from_rgb(0.4, 0.6, 1.0)),
                        ]
                        .spacing(20)
                    ]
                    .spacing(10)
                    .align_x(alignment::Horizontal::Center)
                )
                .padding(10)
                .style(|_theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
                    border: Border {
                        radius: 5.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                // Spacer
                text("").size(20),
                // Connection settings
                column![
                    text("IP Address:").size(16).color(Color::WHITE),
                    text_input("Enter IP", &self.ip_address)
                        .on_input(Message::IpAddressChanged)
                        .padding(10)
                        .width(Length::Fixed(300.0))
                ]
                .spacing(5),
                column![
                    text("Port:").size(16).color(Color::WHITE),
                    text_input("Enter port", &self.port)
                        .on_input(Message::PortChanged)
                        .padding(10)
                        .width(Length::Fixed(300.0))
                ]
                .spacing(5),
                row![
                    text("Protocol:").color(Color::WHITE),
                    text("UDP").color(Color::WHITE),
                    toggler(self.is_tcp).on_toggle(Message::ProtocolToggled),
                    text("TCP").color(Color::WHITE),
                ]
                .spacing(10)
                .align_y(alignment::Vertical::Center),
                text(format!("Target: {}:{}", self.ip_address, self.port))
                    .size(14)
                    .color(Color::from_rgb(0.8, 0.8, 1.0)),
                button(text("Connect").size(18))
                    .on_press(Message::Connect)
                    .padding(15)
                    .width(Length::Fixed(150.0))
            ]
            .spacing(20)
            .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fixed(450.0))
        .padding(30)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
            border: Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

        // Layer the centered panel on top of the full-window background image.
        container(
            Stack::new().push(background).push(
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            ),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

// ==========================================
//           NETWORK LOGIC (WORKER)
// ==========================================

/// Sends a status/speed update to the UI.
fn report(tx: &mpsc::Sender<TestProgress>, phase: &str, mbps: f64, is_download: bool) {
    let _ = tx.send(TestProgress::Update {
        phase: phase.into(),
        mbps,
        is_download,
    });
}

/// Ends the test run and shows `phase` as the final status.
fn finish(tx: &mpsc::Sender<TestProgress>, phase: impl Into<String>) {
    let _ = tx.send(TestProgress::Finished {
        phase: phase.into(),
    });
}

/// Converts a byte count over a duration (in seconds) to megabits per second.
fn to_mbps(bytes: usize, secs: f64) -> f64 {
    if secs > 0.0 {
        (bytes as f64 * 8.0) / (secs * 1_000_000.0)
    } else {
        0.0
    }
}

/// Tracks bytes transferred and sends a live speed update every `REPORT_INTERVAL`.
struct SpeedMeter {
    start: Instant,
    last_report: Instant,
    bytes_interval: usize,
    total_bytes: usize,
}

impl SpeedMeter {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last_report: now,
            bytes_interval: 0,
            total_bytes: 0,
        }
    }

    /// Records `n` transferred bytes, reporting the interval speed if it's time.
    fn add(&mut self, n: usize, tx: &mpsc::Sender<TestProgress>, phase: &str, is_download: bool) {
        self.bytes_interval += n;
        self.total_bytes += n;

        if self.last_report.elapsed() >= REPORT_INTERVAL {
            let mbps = to_mbps(self.bytes_interval, self.last_report.elapsed().as_secs_f64());
            report(tx, phase, mbps, is_download);
            self.bytes_interval = 0;
            self.last_report = Instant::now();
        }
    }

    /// Average speed over the whole test so far.
    fn average_mbps(&self) -> f64 {
        to_mbps(self.total_bytes, self.start.elapsed().as_secs_f64())
    }
}

/// Sends a one-byte command and waits for the server's ACK.
fn tcp_handshake(stream: &mut TcpStream, cmd: u8) -> Result<(), String> {
    stream
        .write_all(&[cmd])
        .and_then(|_| stream.flush())
        .map_err(|e| format!("Failed to send command: {}", e))?;

    // Give the server a moment to process the command.
    thread::sleep(Duration::from_millis(50));

    let mut ack = [0u8; 1];
    match stream.read_exact(&mut ack) {
        Ok(_) if ack[0] == CMD_ACK => Ok(()),
        Ok(_) => Err(format!("Wrong response: 0x{:02x}", ack[0])),
        Err(e) => Err(format!("No ACK: {}", e)),
    }
}

/// Runs the TCP download test followed by the upload test against `addr`.
fn run_tcp_test(addr: &str, tx: &mpsc::Sender<TestProgress>) {
    let mut stream = match TcpStream::connect(addr) {
        Ok(s) => s,
        Err(e) => return finish(tx, format!("Connection Failed: {}", e)),
    };

    // Disable Nagle's algorithm so single-byte commands are sent immediately.
    let _ = stream.set_nodelay(true);

    // Generous timeouts so slow links aren't dropped mid-test.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

    // === DOWNLOAD TEST ===
    report(tx, "Downloading (TCP)...", 0.0, true);

    if let Err(e) = tcp_handshake(&mut stream, CMD_DOWNLOAD) {
        return finish(tx, e);
    }

    let mut buf = vec![0u8; 8192];
    let mut meter = SpeedMeter::new();

    // Read until the server sends DONE, closes, or we pass a safety timeout.
    while meter.start.elapsed() <= Duration::from_secs(8) {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                // Data bytes are never CMD_DONE, so DONE is always the last byte
                // of the final read, even if it arrived together with data.
                if buf[n - 1] == CMD_DONE {
                    meter.add(n - 1, tx, "Downloading...", true);
                    break;
                }
                meter.add(n, tx, "Downloading...", true);
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => continue,
            Err(_) => break,
        }
    }

    report(tx, "Download Complete", meter.average_mbps(), true);

    // Small delay between tests.
    thread::sleep(Duration::from_millis(100));

    // === UPLOAD TEST ===
    report(tx, "Uploading (TCP)...", 0.0, false);

    if let Err(e) = tcp_handshake(&mut stream, CMD_UPLOAD) {
        return finish(tx, e);
    }

    let payload = vec![1u8; 8192];
    let mut meter = SpeedMeter::new();

    while meter.start.elapsed() < UPLOAD_DURATION {
        match stream.write(&payload) {
            Ok(n) => meter.add(n, tx, "Uploading...", false),
            Err(_) => break,
        }
    }

    let _ = stream.flush();

    // Wait for the server's DONE so the timing covers data it actually received.
    // A missing or unexpected reply isn't fatal; we still report the result.
    let mut done = [0u8; 1];
    let _ = stream.read_exact(&mut done);

    report(tx, "Upload Complete", meter.average_mbps(), false);
    finish(tx, "Finished");
}

/// Runs the UDP download test followed by the upload test against `addr`.
fn run_udp_test(addr: &str, tx: &mpsc::Sender<TestProgress>) {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => return finish(tx, format!("UDP Bind Failed: {}", e)),
    };

    // Allow time for the server's ACK to arrive.
    let _ = socket.set_read_timeout(Some(Duration::from_millis(2000)));

    // "Connect" the UDP socket so send/recv default to the server address.
    if let Err(e) = socket.connect(addr) {
        return finish(tx, format!("UDP Connect Failed: {}", e));
    }

    // === DOWNLOAD TEST ===
    report(tx, "Downloading (UDP)...", 0.0, true);

    if socket.send(&[CMD_DOWNLOAD]).is_err() {
        return finish(tx, "Failed to send download command");
    }

    let mut buf = vec![0u8; 2048];

    match socket.recv(&mut buf) {
        Ok(n) if n > 0 && buf[0] == CMD_ACK => {}
        _ => return finish(tx, "No ACK for UDP download"),
    }

    // Shorter timeout while receiving so we can notice when packets stop.
    let _ = socket.set_read_timeout(Some(Duration::from_millis(100)));

    let mut meter = SpeedMeter::new();
    let mut consecutive_timeouts = 0;

    while meter.start.elapsed() < Duration::from_secs(7) {
        match socket.recv(&mut buf) {
            Ok(n) => {
                consecutive_timeouts = 0;

                // DONE arrives as its own single-byte datagram.
                if n == 1 && buf[0] == CMD_DONE {
                    break;
                }

                meter.add(n, tx, "Downloading...", true);
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                consecutive_timeouts += 1;
                // UDP is lossy, so DONE may never arrive. If nothing has come in for
                // a while after the expected test window, assume the test is over.
                if consecutive_timeouts > 20 && meter.start.elapsed() > Duration::from_secs(5) {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    report(tx, "Download Complete", meter.average_mbps(), true);

    thread::sleep(Duration::from_millis(200));

    // === UPLOAD TEST ===
    // UDP upload has no handshake: send the trigger, then stream packets.
    report(tx, "Uploading (UDP)...", 0.0, false);

    let _ = socket.send(&[CMD_UPLOAD]);

    let payload = vec![1u8; 1024];
    let mut meter = SpeedMeter::new();

    while meter.start.elapsed() < UPLOAD_DURATION {
        let sent = if socket.send(&payload).is_ok() {
            payload.len()
        } else {
            0
        };
        meter.add(sent, tx, "Uploading...", false);

        // Brief pause so we don't overrun the send buffer.
        thread::sleep(Duration::from_micros(10));
    }

    report(tx, "Upload Complete", meter.average_mbps(), false);
    finish(tx, "Finished");
}
