use crdts_sandbox_lib;

use bytes::Bytes;

use futures::{future, Sink, SinkExt, Stream, StreamExt};

use tokio::{
    io,
    net::{TcpListener, TcpStream},
    prelude::*,
};

use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:8080".parse::<SocketAddr>()?;

    let stdin = FramedRead::new(io::stdin(), BytesCodec::new());
    let stdin = stdin.map(|i| i.map(|bytes| bytes.freeze()));
    let stdout = FramedWrite::new(io::stdout(), BytesCodec::new());

    connect(&addr, stdin, stdout).await?;

    Ok(())
}

async fn input_handler<'a>(
    mut stdin: impl Stream<Item = Result<Bytes, io::Error>> + Unpin,
    mut sink: FramedWrite<tokio::net::tcp::WriteHalf<'a>, BytesCodec>,
) -> Result<(), std::io::Error> {
    loop {
        let input = stdin.next().await;
        if let Some(Ok(bytes)) = input {
            let bs = bytes;
            let string = std::str::from_utf8(&bs).unwrap();
            let cmd = crdts_sandbox_lib::Command::parse_input(&bs);
            let json: Option<String> =
                cmd.and_then(|c| serde_json::to_string(&c).ok());
            if let Some(json) = json {
                sink.send(Bytes::from(json)).await?;
                sink.flush().await?;
            }
        }
    }
}

async fn connect(
    addr: &SocketAddr,
    stdin: impl Stream<Item = Result<Bytes, io::Error>> + Unpin,
    mut stdout: impl Sink<Bytes, Error = io::Error> + Unpin,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(addr).await?;
    let (r, w) = stream.split();
    let sink: FramedWrite<tokio::net::tcp::WriteHalf, BytesCodec> =
        FramedWrite::new(w, BytesCodec::new());

    let mut stream = FramedRead::new(r, BytesCodec::new())
        .filter_map(|i| match i {
            Ok(i) => future::ready(Some(i.freeze())),
            Err(e) => {
                println!("failed to read from socket; error = {}", e);
                future::ready(None)
            }
        })
        .map(Ok);

    match future::join(input_handler(stdin, sink), stdout.send_all(&mut stream))
        .await
    {
        (Err(e), _) | (_, Err(e)) => Err(e.into()),
        _ => Ok(()),
    }
}
