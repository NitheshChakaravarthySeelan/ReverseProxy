use std::error::Error;
use std::io::{Read, Write};
use std::net::Ipv4Addr;
use std::net::SocketAddrV4;
use std::net::TcpStream;

fn write_to_server(sent: usize, msg: &str, stream: TcpStream) {
    while sent < msg.len() {
        match stream.write(&msg[sent..]) {
            Ok(n) => sent += n,
        }
    }
}

fn client() -> Result<(), Error> {
    /// Setup the socket addr
    let localhost = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(localhost, 8080);

    let mut stream = TcpStream::connect(socket)?;
    stream.set_nonblocking(true)?;
    stream.write_all(b"hello server")?;

    let mut buf = [0u8; 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => println!("got : {:?}", &buf[..n]),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                wait_for_fd();
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn main() {}
