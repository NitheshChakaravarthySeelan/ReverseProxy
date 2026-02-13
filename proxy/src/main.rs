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
async fn handle_client(mut socket: TcpStream) -> tokio::io::Result<()> {
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

        if buffer_window_end(&buf) {
            println!("End of headers found");
            break;
        }
    }
    // connect to the backend server will be more complex need to reimplemnet it later.
    let mut backend_socket = TcpStream::connect("127.0.0.1:81").await?;
    println!("Connected to the backend");

    // taking out the header from the request
    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
        let head = buf[..pos];
        let header = parse_header(&head, pos);
        buf.drain(..pos + 4);
    }

    Ok(())
}

// will change just for end product.
async fn handle_client1(mut client: TcpStream) -> io::Result<()> {
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
