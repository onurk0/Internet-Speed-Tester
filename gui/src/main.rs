use iced::{
    alignment, time,
    widget::{button, column, container, image, row, text, text_input, toggler, Stack},
    Background, Border, Color, Element, Length, Size, Subscription, Task, Theme,
};
use std::io::{Read, Write};
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
    .run_with(|| NetworkConfigApp::new())
}

// --- Protocol Constants ---
const CMD_DOWNLOAD: u8 = 0x01;
const CMD_UPLOAD: u8 = 0x02;
const CMD_ACK: u8 = 0x03;
const CMD_DONE: u8 = 0x04;

// --- MESSAGES ---
#[derive(Debug, Clone)]
pub enum Message {
    IpAddressChanged(String),
    PortChanged(String),
    ProtocolToggled(bool),
    Connect,
    TestStarted,
    TestUpdate(TestProgress),
    TestFinished,
}

#[derive(Debug, Clone)]
pub struct TestProgress {
    pub phase: String,
    pub mbps: f64,
    pub is_download: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Protocol {
    UDP,
    TCP,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::UDP => write!(f, "UDP"),
            Protocol::TCP => write!(f, "TCP"),
        }
    }
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
    receiver: Option<mpsc::Receiver<TestProgress>>,
}

impl NetworkConfigApp {
    fn new() -> (Self, Task<Message>) {
        let background_image = image::Handle::from_path("TCNJ_Speed.jpg");

        (
            Self {
                ip_address: String::from("127.0.0.1"),
                port: String::from("8080"),
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
                self.is_tcp = tcp;
                self.port = if tcp { "8080".into() } else { "7070".into() };
            }
            Message::Connect => {
                if self.is_testing {
                    return Task::none();
                }

                println!("\n=== STARTING SPEED TEST ===");
                self.is_testing = true;
                self.download_speed = 0.0;
                self.upload_speed = 0.0;
                self.current_phase = "Starting...".into();

                let (tx, rx) = mpsc::channel();
                self.receiver = Some(rx);

                let ip = self.ip_address.clone();
                let port = self.port.clone();
                let tcp = self.is_tcp;

                thread::spawn(move || run_speed_test(ip, port, tcp, tx));

                return Task::none();
            }
            Message::TestStarted => {
                if let Some(rx) = &self.receiver {
                    while let Ok(msg) = rx.try_recv() {
                        if msg.mbps < 0.0 {
                            return Task::perform(async {}, |_| Message::TestFinished);
                        }
                        return Task::perform(async { msg }, Message::TestUpdate);
                    }
                }
            }
            Message::TestUpdate(progress) => {
                self.current_phase = progress.phase;
                if progress.is_download {
                    self.download_speed = progress.mbps;
                } else {
                    self.upload_speed = progress.mbps;
                }
            }
            Message::TestFinished => {
                self.is_testing = false;
                self.receiver = None;
                self.current_phase = "Finished".into();
                println!("=== SPEED TEST COMPLETE ===\n");
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.is_testing {
            time::every(Duration::from_millis(100)).map(|_| Message::TestStarted)
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
                text("").size(20),
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

fn run_speed_test(ip: String, port: String, is_tcp: bool, tx: mpsc::Sender<TestProgress>) {
    let addr = format!("{}:{}", ip, port);
    println!("Target: {} ({})", addr, if is_tcp { "TCP" } else { "UDP" });

    if is_tcp {
        run_tcp_test(&addr, tx);
    } else {
        run_udp_test(&addr, tx);
    }
}

fn run_tcp_test(addr: &str, tx: mpsc::Sender<TestProgress>) {
    println!("[CLIENT-TCP] Connecting to {}...", addr);

    let mut stream = match TcpStream::connect(addr) {
        Ok(s) => {
            println!("[CLIENT-TCP] Connected successfully!");
            s
        }
        Err(e) => {
            println!("[CLIENT-TCP] ERROR: Connection failed: {}", e);
            let _ = tx.send(TestProgress {
                phase: format!("Connection Failed: {}", e),
                mbps: -1.0,
                is_download: true,
            });
            return;
        }
    };

    // CRITICAL: Disable Nagle's algorithm and set generous timeouts
    if let Err(e) = stream.set_nodelay(true) {
        println!("[CLIENT-TCP] ERROR: Failed to set nodelay: {}", e);
    } else {
        println!("[CLIENT-TCP] Set TCP_NODELAY = true");
    }

    // Use longer timeouts to prevent premature connection closure
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

    // === DOWNLOAD TEST ===
    println!("\n[CLIENT-TCP] === STARTING DOWNLOAD TEST ===");
    let _ = tx.send(TestProgress {
        phase: "Downloading (TCP)...".into(),
        mbps: 0.0,
        is_download: true,
    });

    println!(
        "[CLIENT-TCP-DL] Step 1: Sending DOWNLOAD command (0x{:02x})",
        CMD_DOWNLOAD
    );
    if let Err(e) = stream.write_all(&[CMD_DOWNLOAD]) {
        println!("[CLIENT-TCP-DL] ERROR: Failed to send command: {}", e);
        let _ = tx.send(TestProgress {
            phase: "Failed to send download command".into(),
            mbps: -1.0,
            is_download: true,
        });
        return;
    }
    if let Err(e) = stream.flush() {
        println!("[CLIENT-TCP-DL] ERROR: Failed to flush: {}", e);
        return;
    }

    // Small delay to let server process the command
    thread::sleep(Duration::from_millis(50));

    println!("[CLIENT-TCP-DL] Step 2: Waiting for ACK...");
    let mut ack = [0u8; 1];
    match stream.read_exact(&mut ack) {
        Ok(_) if ack[0] == CMD_ACK => {
            println!("[CLIENT-TCP-DL] Step 3: Received ACK (0x{:02x})", ack[0]);
        }
        Ok(_) => {
            println!(
                "[CLIENT-TCP-DL] ERROR: Expected ACK but got 0x{:02x}",
                ack[0]
            );
            let _ = tx.send(TestProgress {
                phase: format!("Wrong response: 0x{:02x}", ack[0]),
                mbps: -1.0,
                is_download: true,
            });
            return;
        }
        Err(e) => {
            println!("[CLIENT-TCP-DL] ERROR: Failed to receive ACK: {}", e);
            let _ = tx.send(TestProgress {
                phase: format!("No ACK: {}", e),
                mbps: -1.0,
                is_download: true,
            });
            return;
        }
    }

    println!("[CLIENT-TCP-DL] Step 4: Starting to receive data...");

    // Use a larger buffer and non-blocking reads with timeout handling
    let mut buf = vec![0u8; 8192];
    let start = Instant::now();
    let mut last_report = start;
    let mut bytes_interval = 0;
    let mut total_bytes = 0;

    loop {
        if start.elapsed() > Duration::from_secs(8) {
            println!("[CLIENT-TCP-DL] WARNING: Test duration exceeded");
            break;
        }

        match stream.read(&mut buf) {
            Ok(0) => {
                println!("[CLIENT-TCP-DL] Connection closed by server");
                break;
            }
            Ok(n) => {
                // Check if this is just the DONE signal
                if n == 1 && buf[0] == CMD_DONE {
                    println!(
                        "[CLIENT-TCP-DL] Step 5: Received DONE signal (0x{:02x})",
                        buf[0]
                    );
                    break;
                }

                bytes_interval += n;
                total_bytes += n;

                // Report speed every 500ms
                if last_report.elapsed() >= Duration::from_millis(500) {
                    let mbps = (bytes_interval as f64 * 8.0)
                        / (last_report.elapsed().as_secs_f64() * 1_000_000.0);
                    println!(
                        "[CLIENT-TCP-DL] Interval speed: {:.2} Mbps ({} bytes)",
                        mbps, total_bytes
                    );
                    let _ = tx.send(TestProgress {
                        phase: "Downloading...".into(),
                        mbps,
                        is_download: true,
                    });
                    bytes_interval = 0;
                    last_report = Instant::now();
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // This shouldn't happen with blocking reads, but handle it
                continue;
            }
            Err(e) => {
                println!("[CLIENT-TCP-DL] ERROR: Read error: {}", e);
                break;
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total_mbps = if elapsed > 0.0 {
        (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    println!("[CLIENT-TCP-DL] Step 6: Download complete");
    println!(
        "[CLIENT-TCP-DL]   - Total: {} bytes ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_000_000.0
    );
    println!("[CLIENT-TCP-DL]   - Duration: {:.2} seconds", elapsed);
    println!("[CLIENT-TCP-DL]   - Average: {:.2} Mbps", total_mbps);

    let _ = tx.send(TestProgress {
        phase: "Download Complete".into(),
        mbps: total_mbps,
        is_download: true,
    });

    // Small delay between tests
    thread::sleep(Duration::from_millis(100));

    // === UPLOAD TEST ===
    println!("\n[CLIENT-TCP] === STARTING UPLOAD TEST ===");
    let _ = tx.send(TestProgress {
        phase: "Uploading (TCP)...".into(),
        mbps: 0.0,
        is_download: false,
    });

    println!(
        "[CLIENT-TCP-UL] Step 1: Sending UPLOAD command (0x{:02x})",
        CMD_UPLOAD
    );
    if let Err(e) = stream.write_all(&[CMD_UPLOAD]) {
        println!("[CLIENT-TCP-UL] ERROR: Failed to send command: {}", e);
        return;
    }
    let _ = stream.flush();

    thread::sleep(Duration::from_millis(50));

    println!("[CLIENT-TCP-UL] Step 2: Waiting for ACK...");
    match stream.read_exact(&mut ack) {
        Ok(_) if ack[0] == CMD_ACK => {
            println!("[CLIENT-TCP-UL] Step 3: Received ACK (0x{:02x})", ack[0]);
        }
        Ok(_) => {
            println!(
                "[CLIENT-TCP-UL] ERROR: Expected ACK but got 0x{:02x}",
                ack[0]
            );
            return;
        }
        Err(e) => {
            println!("[CLIENT-TCP-UL] ERROR: Failed to receive ACK: {}", e);
            return;
        }
    }

    println!("[CLIENT-TCP-UL] Step 4: Starting to send data...");

    let payload = vec![1u8; 8192];
    let start = Instant::now();
    let mut last_report = start;
    let mut bytes_interval = 0;
    let mut total_bytes = 0;

    while start.elapsed() < Duration::from_secs(5) {
        match stream.write(&payload) {
            Ok(n) => {
                bytes_interval += n;
                total_bytes += n;
            }
            Err(e) => {
                println!("[CLIENT-TCP-UL] ERROR: Write failed: {}", e);
                break;
            }
        }

        if last_report.elapsed() >= Duration::from_millis(500) {
            let mbps =
                (bytes_interval as f64 * 8.0) / (last_report.elapsed().as_secs_f64() * 1_000_000.0);
            println!("[CLIENT-TCP-UL] Interval speed: {:.2} Mbps", mbps);
            let _ = tx.send(TestProgress {
                phase: "Uploading...".into(),
                mbps,
                is_download: false,
            });
            bytes_interval = 0;
            last_report = Instant::now();
        }
    }

    let _ = stream.flush();

    println!("[CLIENT-TCP-UL] Step 5: Waiting for DONE signal...");
    match stream.read_exact(&mut ack) {
        Ok(_) if ack[0] == CMD_DONE => {
            println!(
                "[CLIENT-TCP-UL] Step 6: Received DONE signal (0x{:02x})",
                ack[0]
            );
        }
        Ok(_) => {
            println!(
                "[CLIENT-TCP-UL] WARNING: Expected DONE but got 0x{:02x}",
                ack[0]
            );
        }
        Err(e) => {
            println!("[CLIENT-TCP-UL] WARNING: Failed to receive DONE: {}", e);
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total_mbps = if elapsed > 0.0 {
        (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    println!("[CLIENT-TCP-UL] Step 7: Upload complete");
    println!(
        "[CLIENT-TCP-UL]   - Total: {} bytes ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_000_000.0
    );
    println!("[CLIENT-TCP-UL]   - Average: {:.2} Mbps", total_mbps);

    let _ = tx.send(TestProgress {
        phase: "Upload Complete".into(),
        mbps: total_mbps,
        is_download: false,
    });

    println!("[CLIENT-TCP] === ALL TESTS COMPLETE ===");

    // Signal finish
    let _ = tx.send(TestProgress {
        phase: "Done".into(),
        mbps: -1.0,
        is_download: true,
    });
}

fn run_udp_test(addr: &str, tx: mpsc::Sender<TestProgress>) {
    println!("[CLIENT-UDP] Binding socket...");

    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => {
            println!("[CLIENT-UDP] Socket bound successfully");
            s
        }
        Err(e) => {
            println!("[CLIENT-UDP] ERROR: Bind failed: {}", e);
            let _ = tx.send(TestProgress {
                phase: format!("UDP Bind Failed: {}", e),
                mbps: -1.0,
                is_download: true,
            });
            return;
        }
    };

    // Longer timeout for UDP
    let _ = socket.set_read_timeout(Some(Duration::from_millis(2000)));

    if socket.connect(addr).is_err() {
        println!("[CLIENT-UDP] ERROR: Connect failed");
        return;
    }
    println!("[CLIENT-UDP] Connected to {}", addr);

    // === DOWNLOAD TEST ===
    println!("\n[CLIENT-UDP] === STARTING DOWNLOAD TEST ===");
    let _ = tx.send(TestProgress {
        phase: "Downloading (UDP)...".into(),
        mbps: 0.0,
        is_download: true,
    });

    println!(
        "[CLIENT-UDP-DL] Step 1: Sending DOWNLOAD trigger (0x{:02x})",
        CMD_DOWNLOAD
    );
    if socket.send(&[CMD_DOWNLOAD]).is_err() {
        println!("[CLIENT-UDP-DL] ERROR: Failed to send trigger");
        return;
    }

    println!("[CLIENT-UDP-DL] Step 2: Waiting for ACK...");
    let mut buf = vec![0u8; 2048];

    match socket.recv(&mut buf) {
        Ok(n) if n > 0 && buf[0] == CMD_ACK => {
            println!("[CLIENT-UDP-DL] Step 3: Received ACK (0x{:02x})", buf[0]);
        }
        Ok(n) => {
            println!(
                "[CLIENT-UDP-DL] ERROR: Expected ACK but got {} bytes, first byte: 0x{:02x}",
                n, buf[0]
            );
            let _ = tx.send(TestProgress {
                phase: "No ACK for UDP download".into(),
                mbps: -1.0,
                is_download: true,
            });
            return;
        }
        Err(e) => {
            println!("[CLIENT-UDP-DL] ERROR: Failed to receive ACK: {}", e);
            let _ = tx.send(TestProgress {
                phase: "No ACK for UDP download".into(),
                mbps: -1.0,
                is_download: true,
            });
            return;
        }
    }

    println!("[CLIENT-UDP-DL] Step 4: Starting to receive packets...");

    // Shorter timeout for receiving packets
    let _ = socket.set_read_timeout(Some(Duration::from_millis(100)));

    let start = Instant::now();
    let mut last_report = start;
    let mut bytes_interval = 0;
    let mut total_bytes = 0;
    let mut packets = 0;
    let mut consecutive_timeouts = 0;

    while start.elapsed() < Duration::from_secs(7) {
        match socket.recv(&mut buf) {
            Ok(n) => {
                consecutive_timeouts = 0;

                // Check for DONE signal
                if n == 1 && buf[0] == CMD_DONE {
                    println!(
                        "[CLIENT-UDP-DL] Step 5: Received DONE signal (0x{:02x})",
                        buf[0]
                    );
                    break;
                }

                bytes_interval += n;
                total_bytes += n;
                packets += 1;

                if last_report.elapsed() >= Duration::from_millis(500) {
                    let mbps = (bytes_interval as f64 * 8.0)
                        / (last_report.elapsed().as_secs_f64() * 1_000_000.0);
                    println!(
                        "[CLIENT-UDP-DL] Speed: {:.2} Mbps ({} packets, {} bytes total)",
                        mbps, packets, total_bytes
                    );
                    let _ = tx.send(TestProgress {
                        phase: "Downloading...".into(),
                        mbps,
                        is_download: true,
                    });
                    bytes_interval = 0;
                    last_report = Instant::now();
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                consecutive_timeouts += 1;
                // If we haven't received anything for a while, the test might be over
                if consecutive_timeouts > 20 && start.elapsed() > Duration::from_secs(5) {
                    println!("[CLIENT-UDP-DL] No more packets received, ending test");
                    break;
                }
                continue;
            }
            Err(e) => {
                println!("[CLIENT-UDP-DL] ERROR: Receive error: {}", e);
                break;
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total_mbps = if elapsed > 0.0 {
        (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    println!("[CLIENT-UDP-DL] Step 6: Download complete");
    println!("[CLIENT-UDP-DL]   - Received {} packets", packets);
    println!(
        "[CLIENT-UDP-DL]   - Total: {} bytes ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_000_000.0
    );
    println!("[CLIENT-UDP-DL]   - Average: {:.2} Mbps", total_mbps);

    let _ = tx.send(TestProgress {
        phase: "Download Complete".into(),
        mbps: total_mbps,
        is_download: true,
    });

    thread::sleep(Duration::from_millis(200));

    // === UPLOAD TEST ===
    println!("\n[CLIENT-UDP] === STARTING UPLOAD TEST ===");
    let _ = tx.send(TestProgress {
        phase: "Uploading (UDP)...".into(),
        mbps: 0.0,
        is_download: false,
    });

    println!(
        "[CLIENT-UDP-UL] Step 1: Sending UPLOAD trigger (0x{:02x})",
        CMD_UPLOAD
    );
    let _ = socket.send(&[CMD_UPLOAD]);

    println!("[CLIENT-UDP-UL] Step 2: Starting to send packets...");

    let payload = vec![1u8; 1024];
    let start = Instant::now();
    let mut last_report = start;
    let mut bytes_interval = 0;
    let mut total_bytes = 0;
    let mut packets = 0;

    while start.elapsed() < Duration::from_secs(5) {
        if socket.send(&payload).is_ok() {
            bytes_interval += payload.len();
            total_bytes += payload.len();
            packets += 1;
        }

        thread::sleep(Duration::from_micros(10));

        if last_report.elapsed() >= Duration::from_millis(500) {
            let mbps =
                (bytes_interval as f64 * 8.0) / (last_report.elapsed().as_secs_f64() * 1_000_000.0);
            println!("[CLIENT-UDP-UL] Interval speed: {:.2} Mbps", mbps);
            let _ = tx.send(TestProgress {
                phase: "Uploading...".into(),
                mbps,
                is_download: false,
            });
            bytes_interval = 0;
            last_report = Instant::now();
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total_mbps = if elapsed > 0.0 {
        (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0)
    } else {
        0.0
    };

    println!("[CLIENT-UDP-UL] Step 3: Upload complete");
    println!("[CLIENT-UDP-UL]   - Sent {} packets", packets);
    println!(
        "[CLIENT-UDP-UL]   - Total: {} bytes ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_000_000.0
    );
    println!("[CLIENT-UDP-UL]   - Average: {:.2} Mbps", total_mbps);

    let _ = tx.send(TestProgress {
        phase: "Upload Complete".into(),
        mbps: total_mbps,
        is_download: false,
    });

    println!("[CLIENT-UDP] === ALL TESTS COMPLETE ===");

    // Signal finish
    let _ = tx.send(TestProgress {
        phase: "Done".into(),
        mbps: -1.0,
        is_download: true,
    });
}
