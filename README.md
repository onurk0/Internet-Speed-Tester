# Internet Speed Tester

A small Rust-based networking project that measures upload and download speed between two machines over TCP or UDP.

## Overview

This project contains two parts:

- `server/` — listens for incoming test connections and handles both TCP and UDP throughput tests
- `gui/` — an `iced` desktop application that connects to a remote server, starts the test, and displays live results in Mbps

The app is designed for a real two-machine setup on the same network. It is not meant to be a realistic benchmark when both programs run on the same machine.

## Features

- TCP and UDP test modes
- Download and upload measurement
- Live status updates during the test
- Simple desktop GUI for entering target host information
- Custom protocol with ACK and DONE signaling between client and server

## Project structure

```text
.
├── README.md
├── gui/
│   ├── Cargo.toml
│   ├── TCNJ_Speed.jpg
│   └── src/
│       └── main.rs
└── server/
    ├── Cargo.toml
    └── src/
        └── main.rs
```

## Requirements

Before running the project, make sure you have:

- Rust installed with Cargo
- Two devices on the same network
- A reachable server machine from the machine running the GUI

## How it works

The server listens on fixed ports:

| Protocol | Port | Listener |
| --- | ---: | --- |
| TCP | 8080 | `0.0.0.0:8080` |
| UDP | 7070 | `0.0.0.0:7070` |

The GUI client sends commands to the target server to run a download test and then an upload test. It updates the status text and displays the measured speeds in real time.

## Running the application

### 1. Start the server

Run this on the machine that will act as the speed test host:

```bash
cd server
cargo run
```

You should see output similar to:

```text
=== Starting Speed Test Server ===
TCP Listening on port 8080
UDP Listening on port 7070
```

### 2. Start the GUI client

On the other machine, run:

```bash
cd gui
cargo run
```

This opens the desktop window where you can enter the target server information.

### 3. Enter the server IP and run the test

Use the server machine's actual IP address, not `127.0.0.1`, unless both programs are running on the same machine.

Find the server machine IP:

- Linux/macOS:

```bash
ip addr
```

- Windows:

```powershell
ipconfig
```

Then in the GUI:

1. Enter the server IP address
2. Set the port to the matching protocol port
   - TCP: `8080`
   - UDP: `7070`
3. Use the protocol toggle to switch between UDP and TCP
4. Click `Connect`

## Protocol notes

The app automatically changes the port when you toggle the protocol:

- TCP uses port `8080`
- UDP uses port `7070`

The GUI defaults to `127.0.0.1` and `8080`, but for a real test you should replace that with your server's LAN IP.

## Important notes

- The server must be running before the client connects.
- The client expects the server to be reachable over the network.
- Do not use loopback or the same machine for a proper test; results will be misleading.
- The GUI loads a background image file from the `gui` directory. Running the app from the expected project layout helps avoid path issues.

## Troubleshooting

If the connection fails:

- confirm the server is still running
- verify the target IP is correct
- ensure the port matches the selected protocol
- check that your firewall is not blocking traffic on `8080` or `7070`

If the test stalls or never starts:

- verify both machines are on the same network or properly routed
- make sure the server is listening on the same protocol/port the GUI is using
- ensure the client is not accidentally targeting `127.0.0.1` when it should use the remote host IP

## Quick start

```bash
# Server machine
cd server
cargo run

# Client machine
cd gui
cargo run
```

Then enter the server IP, choose the protocol, and click `Connect`.

## License

This project is currently without a formal license file in the repository.
