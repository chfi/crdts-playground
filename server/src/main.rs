use crdts_sandbox_lib::document::{
    Command, DocActor, DocResponse, Document, DocumentOp,
};

use futures::{lock, FutureExt, Sink, SinkExt, Stream, StreamExt};

use warp::{ws::Message, Filter};

use tokio::{
    // io::{AsyncBufRead, AsyncBufReadExt},
    net::{TcpListener, TcpStream},
    prelude::*,
};

use bytes::Bytes;
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

fn parse_command(msg: Message) -> Option<Command> {
    if msg.is_binary() {
        let bytes = msg.as_bytes();
        Command::from_bytes(&bytes)
    } else {
        None
    }
}

fn command_into_message(cmd: Command) -> Message {
    let bytes_vec = cmd.to_bytes().unwrap();
    Message::binary(bytes_vec)
}

fn docresp_into_message(resp: DocResponse) -> Message {
    let bytes = bincode::serialize(&resp).unwrap();
    Message::binary(bytes)
}

async fn handle_connection_wrapper(
    state: Arc<lock::Mutex<State>>,
    // mut sink: impl Sink<Message, Error = warp::Error> + Unpin,
    sink: impl Sink<Message, Error = warp::Error> + Unpin,
    stream: impl Stream<Item = Result<Message, warp::Error>> + Unpin,
) {
    let _ = handle_connection(state, sink, stream).await;
}

async fn handle_connection(
    state: Arc<lock::Mutex<State>>,
    // mut sink: impl Sink<Message, Error = warp::Error> + Unpin,
    mut sink: impl Sink<Message, Error = warp::Error> + Unpin,
    mut stream: impl Stream<Item = Result<Message, warp::Error>> + Unpin,
) -> Result<(), Box<dyn std::error::Error>> {
    while let Some(Ok(msg)) = stream.next().await {
        if let Some(cmd) = parse_command(msg) {
            let mut state = state.lock().await;
            match cmd {
                Command::GetDocument => {
                    let resp = DocResponse::Document(state.doc.clone());
                    let msg = docresp_into_message(resp);
                    sink.send(msg).await?;
                    sink.flush().await?;
                }
                Command::GetRecord { key } => {
                    let record_ctx = state.doc.get_record(key);
                    let resp = DocResponse::Record(record_ctx);
                    let msg = docresp_into_message(resp);
                    sink.send(msg).await?;
                    sink.flush().await?;
                }
                Command::GetReadCtx => {
                    let ctx = state.doc.get_read_ctx();
                    let resp = DocResponse::ReadCtx(ctx);
                    let msg = docresp_into_message(resp);
                    sink.send(msg).await?;
                    sink.flush().await?;
                }
                Command::Add {
                    add_ctx,
                    key,
                    content,
                } => {
                    let content = Vec::from(content.as_bytes());
                    let op =
                        state.doc.update_record(key, add_ctx, |set, ctx| {
                            set.add(content, ctx)
                        });
                    state.ops.push(op.clone());
                    state.doc.apply(op);
                }
                Command::Apply { op } => {
                    state.ops.push(op.clone());
                    state.doc.apply(op);
                }
            }
        }
    }

    Ok(())
}

/*
async fn websocket_connected(ws: warp::ws::WebSocket) {
    let (tx, rx) = ws.split();

    tokio::spawn(async move {

    });

}
*/

#[tokio::main]
async fn main() {
    // pretty_env_logger::init();

    // let state = Arc::new(Mutex::new(State::new()));

    let echo = warp::path("echo")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            // And then our closure will be called when it completes...
            ws.on_upgrade(|websocket| {
                let (tx, rx) = websocket.split();

                rx.inspect(|msg| {
                    if let Ok(m) = msg {
                        if let Ok(string) = m.to_str() {
                            println!("received: {}", string);
                        }
                    }
                })
                .forward(tx)
                .map(|result| {
                    if let Err(e) = result {
                        eprintln!("websocket error: {:?}", e);
                    }
                })
            })
        });

    let state = Arc::new(lock::Mutex::new(State::new()));
    let state = warp::any().map(move || state.clone());

    let service = warp::path("service").and(warp::ws()).and(state).map(
        |ws: warp::ws::Ws, state| {
            ws.on_upgrade(move |websocket| {
                let (tx, rx) = websocket.split();
                handle_connection_wrapper(state, tx, rx)
            })
        },
    );

    // let routes = index

    /*
    let routes = warp::path("echo")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            // And then our closure will be called when it completes...
            ws.on_upgrade(|websocket| {
                let (tx, rx) = websocket.split();

                rx.inspect(|msg| {
                    if let Ok(m) = msg {
                        if let Ok(string) = m.to_str() {
                            println!("received: {}", string);
                        }
                    }
                })
                .forward(tx)
                .map(|result| {
                    if let Err(e) = result {
                        eprintln!("websocket error: {:?}", e);
                    }
                })
            })
        });
    */

    // warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    warp::serve(service).run(([127, 0, 0, 1], 3030)).await;
}

// #[tokio::main]
async fn old_main() -> Result<(), Box<dyn std::error::Error>> {
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
                    } else {
                        out_buf.extend_from_slice(&buf[0..n]);
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
