use crdts_sandbox_lib;

use tokio::{
    net::{TcpListener, TcpStream},
    prelude::*,
};

use serde_json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    // println!("Hello, world!");

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            println!("client connected");

            loop {
                let n = match socket.read(&mut buf).await {
                    // Socket closed
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                println!("received {} bytes", n);
                if n > 0 {
                    if let Ok(s) = std::str::from_utf8(&buf[0..n]) {
                        println!("{}", s);
                    }
                }

                if let Err(e) = socket.write_all(&buf[0..n]).await {
                    eprintln!("failed to read from socket; err = {:?}", e);
                    return;
                }
            }
        });
    }
}
