use crdts_sandbox_lib::{DocActor, Document, DocumentOp, RecordEntry};

use bytes::{Bytes, BytesMut};

use std::net::SocketAddr;

use std::{
    io::{stdout, Write},
    time::Duration,
};

use futures::{future, future::FutureExt, Sink, SinkExt, Stream, StreamExt};

use futures_timer::Delay;

use tokio::{
    io,
    net::{TcpListener, TcpStream},
    prelude::*,
    sync::mpsc,
};

use tokio_util::codec::{BytesCodec, Framed, FramedRead, FramedWrite};

use crossterm::{
    cursor,
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode,
    },
    execute, queue, style,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal,
    terminal::ClearType,
    ExecutableCommand,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MenuCommand {
    Up,
    Down,
    Enter,
    Quit,
}

impl MenuCommand {
    fn from_keycode(key_code: KeyCode) -> Option<Self> {
        match key_code {
            KeyCode::Up => Some(Self::Up),
            KeyCode::Down => Some(Self::Down),
            KeyCode::Enter => Some(Self::Enter),
            KeyCode::Esc => Some(Self::Quit),
            _ => None,
        }
    }
    fn from_event(event: &Event) -> Option<Self> {
        match event {
            Event::Key(key_event) => Self::from_keycode(key_event.code),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct ClientState {
    actor: Option<DocActor>,
    document: Option<Document>,
    streams: Framed<TcpStream, BytesCodec>,
}

impl ClientState {
    fn new(stream: TcpStream) -> Self {
        let streams = Framed::new(stream, BytesCodec::new());
        ClientState {
            actor: None,
            document: None,
            streams,
        }
    }
}

#[derive(Debug)]
struct MenuState {
    pub index: usize,
    items: Vec<String>,
    pub cmd_channel: mpsc::Receiver<MenuCommand>,
}

impl MenuState {
    fn command_menu(chn: mpsc::Receiver<MenuCommand>) -> Self {
        let items = vec![
            "Get document".into(),
            "Get record by key".into(),
            "Get document read context".into(),
            "Update a record".into(),
            "Request an actor from server".into(),
        ];
        MenuState {
            index: 3,
            items,
            cmd_channel: chn,
        }
    }

    fn print_menu<W: Write>(&self, write: &mut W) -> crossterm::Result<()> {
        execute!(write, cursor::SavePosition)?;

        for (i, item) in self.items.iter().enumerate() {
            if i == self.index {
                println!("{}  * {}", style::Attribute::Bold, item);
            } else {
                println!("{}  * {}", style::Attribute::NormalIntensity, item);
            }
            execute!(write, cursor::MoveToColumn(0))?;
        }

        execute!(
            write,
            cursor::RestorePosition,
            style::SetAttribute(style::Attribute::NormalIntensity)
        )
    }

    fn min_index(&self) -> usize {
        0
    }

    fn max_index(&self) -> usize {
        self.items.len() - 1
    }

    fn previous(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }

    fn next(&mut self) {
        if self.index < self.max_index() {
            self.index += 1;
        }
    }

    fn apply_command(&mut self, cmd: MenuCommand) {
        match cmd {
            MenuCommand::Up => self.previous(),
            MenuCommand::Down => self.next(),
            MenuCommand::Enter => (),
            _ => (),
        }
    }
}

async fn events_loop(
    tx: mpsc::Sender<MenuCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = EventStream::new();
    let mut tx: mpsc::Sender<MenuCommand> = tx;

    loop {
        let mut delay = Delay::new(Duration::from_millis(1_000)).fuse();
        let mut event = reader.next().fuse();

        futures::select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {

                        if let Some(cmd) = MenuCommand::from_event(&event) {
                            tx.send(cmd).await?;
                        }

                        if event == Event::Key(KeyCode::Esc.into()) {
                            break;
                        }
                    }
                    Some(Err(e)) => eprintln!("Error: {:?}\r", e),
                    None => break,
                }
            }
        };
    }
    Ok(())
}

fn print_in_corner<W: Write>(
    row_offset: u16,
    s: &str,
    write: &mut W,
) -> crossterm::Result<()> {
    execute!(
        write,
        cursor::SavePosition,
        cursor::MoveTo(30, 30 + row_offset)
    )?;
    print!("{}", s);
    execute!(write, cursor::RestorePosition)?;
    Ok(())
}

async fn wait() {
    Delay::new(Duration::from_millis(1_000)).await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel(10);

    let mut menu_state = MenuState::command_menu(rx);

    let mut stdout = stdout();

    execute!(
        stdout,
        cursor::MoveUp(10),
        terminal::Clear(ClearType::FromCursorDown),
    )?;

    let (top, left) = cursor::position()?;

    terminal::enable_raw_mode()?;

    let ev_loop_handle = tokio::spawn(async move {
        let _ = events_loop(tx).await;
    });

    menu_state.print_menu(&mut stdout)?;

    while let Some(cmd) = menu_state.cmd_channel.recv().await {
        match cmd {
            MenuCommand::Quit => break,
            _ => {
                menu_state.apply_command(cmd);
                print_in_corner(
                    1,
                    &format!("index:  {}", menu_state.index),
                    &mut stdout,
                )?;
                menu_state.print_menu(&mut stdout)?;
            }
        }
    }

    terminal::disable_raw_mode()?;
    Ok(())
}

async fn client_main() -> Result<(), Box<dyn std::error::Error>> {
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
