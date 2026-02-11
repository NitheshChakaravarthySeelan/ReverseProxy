mod check_valid;
use std::io;

use check_valid::{buffer_window_end, is_valid_read_len};
use proxy::parse_header;
use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpSocket, TcpStream};

/// 1. Read all data from the client connection.
/// 2. Write all that data to the backend connection.
/// 3. Read all data from the backend connection.
/// 4. Write all that data to the client connection.
async fn process_socket(mut socket: TcpStream, mut backend: TcpStream) -> tokio::io::Result<()> {
    // function to be taken after the connection
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut temp = [0u8; 1024];

    loop {
        let n = socket.read(&mut temp).await?; // need to look into this line

        if !is_valid_read_len(n) {
            // client disconnect
            return Ok(());
        }

        buf.extend_from_slice(&temp[..n]);

        if !buffer_window_end(buf) {
            temp = [0; 1024];
            continue;
        }

        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let (headers, header_end) = parse_header(buf, pos + 4);
            buf.drain(..header_end);
        }

        // Now we need to validate the header in the lib
    }
    Ok(())
}

async fn handle_client(mut client: TcpStream) -> io::Result<()> {
    let mut backend = TcpStream::connect("127.0.0.1:81").await?;
    copy_bidirectional(&mut client, &mut backend).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:80").await?;

    loop {
        /// Accepting connection async socket should be mut cause we will be writing back the
        /// response in the socket.
        /// it would give socket and addr
        let (mut socket, _) = listener.accept().await;

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}
