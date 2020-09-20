use crdts_sandbox_lib::{Command, DocActor, DocResponse, Document, DocumentOp};

use tokio::{
    net::{TcpListener, TcpStream},
    prelude::*,
};

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

type Db = Arc<Mutex<Document>>;
type LatestActor = Arc<Mutex<DocActor>>;

struct State {
    doc: Document,
    ops: Vec<DocumentOp>,
    latest_actor: DocActor,
    clients: HashSet<DocActor>,
}

impl State {
    fn new() -> Self {
        State {
            doc: Document::example(),
            ops: Vec::new(),
            latest_actor: 0,
            clients: HashSet::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let state = Arc::new(Mutex::new(State::new()));

    loop {
        let (mut socket, _) = listener.accept().await?;

        let state = Arc::clone(&state);

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
                                let state = state.lock().unwrap();
                                println!("received GetDocument");
                                let doc =
                                    DocResponse::Document(state.doc.clone());
                                match bincode::serialize(&doc) {
                                    Ok(bytes) => {
                                        out_buf.extend_from_slice(&bytes);
                                        println!("Transmitting document");
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "error serializing doc: {:?}",
                                            e
                                        );
                                    }
                                }
                            }
                            Command::GetRecord { key } => {
                                let state = state.lock().unwrap();
                                let record_ctx = state.doc.get_record(key);
                                let record = DocResponse::Record(record_ctx);
                                let bytes =
                                    bincode::serialize(&record).unwrap();
                                out_buf.extend_from_slice(&bytes);
                                println!("received GetRecord");
                            }
                            Command::GetReadCtx => {
                                let state = state.lock().unwrap();
                                let ctx = state.doc.get_read_ctx();
                                let ctx = DocResponse::ReadCtx(ctx);
                                let bytes = bincode::serialize(&ctx).unwrap();
                                out_buf.extend_from_slice(&bytes);
                                println!("received GetReadCtx");
                            }
                            Command::Add {
                                add_ctx,
                                key,
                                content,
                            } => {
                                let content = Vec::from(content.as_bytes());
                                let mut state = state.lock().unwrap();
                                let op = state.doc.update_record(
                                    key,
                                    add_ctx,
                                    |set, ctx| set.add(content, ctx),
                                );
                                state.ops.push(op.clone());
                                state.doc.apply(op);
                            }
                            Command::Apply { op } => {
                                let mut state = state.lock().unwrap();
                                state.ops.push(op.clone());
                                state.doc.apply(op);
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
