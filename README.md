# Internet Speed Tester

An internet speed tester built in Rust!

This is a fun little project to demonstrate network and socket programming.
In this project, you can pick your desired transport protocol (TCP or UDP),
and test your internet speed with another device.

To run, you need:
1. Install Rust on your machines
2. Download and extract the source code on two seperate machines.
3. Open the server source on one a machine and the gui on the other.
4. Compile and run the server before the gui.
- In **server**: `cargo run`
5. Do the same for the gui
-  In **gui**: `cargo run`

**_When the gui shows up, you can write the IP of the machine you want to test
speeds with. It is highly recommeneded you do not test on the same machine as
inaccurate results will occur._**

To find the ip address of your machine:
- For Linux and MacOS `ip addr`
- For Windows `ipconfig`

Enter the IP address, pick your desired protocol, and hit test speed!
