use crdts_sandbox_lib::{Command, Document, DocumentOp};

use tokio::{
    net::{TcpListener, TcpStream},
    prelude::*,
};

use std::sync::{Arc, Mutex};

use serde_json;

type Db = Arc<Mutex<Document>>;

// async fn process(socket: TcpStream, db: Db) {

// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let db = Arc::new(Mutex::new(Document::example()));

    loop {
        let (mut socket, _) = listener.accept().await?;

        let db = Arc::clone(&db);
        tokio::spawn(async move {
            let mut buf = [0; 1024];
            let mut out_buf: Vec<u8> = Vec::with_capacity(1024);

            println!("client connected");

            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                println!("received {} bytes", n);
                if n > 0 {
                    if let Some(cmd) = Command::from_bytes(&buf[0..n]) {
                        match cmd {
                            Command::GetDocument => {
                                println!("received GetDocument");
                                let db = db.lock().unwrap();
                                let doc_json =
                                    serde_json::to_string(&db.records).unwrap();
                                let doc_bytes = doc_json.as_bytes();
                                out_buf.extend_from_slice(doc_bytes);
                            }
                            Command::GetRecord { key } => {
                                println!("received GetRecord {}", key);
                            }
                            Command::GetReadCtx => {
                                println!("received GetReadCtx");
                            }
                            Command::Update {
                                actor,
                                key,
                                content,
                            } => {
                                println!("received Update: actor {}, key {}, content {}", actor, key, content);
                            }
                            Command::RequestActor => {
                                println!("received RequestActor");
                            }
                        }
                    }
                }

                if !out_buf.is_empty() {
                    if let Err(e) = socket.write_all(&out_buf[..]).await {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                    out_buf.clear();
                }
                /*
                if let Err(e) = socket.write_all(&buf[0..n]).await {
                    eprintln!("failed to read from socket; err = {:?}", e);
                    return;
                }
                */
            }
        });
    }
}
