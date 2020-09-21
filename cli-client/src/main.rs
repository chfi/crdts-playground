use crdts_sandbox_lib::document::{
    Command, DocActor, DocResponse, Document, DocumentOp, RecordEntry,
};

use bytes::Bytes;

use std::{
    io::{stdout, Write},
    net::SocketAddr,
    time::Duration,
};

use futures::{future::FutureExt, Sink, SinkExt, StreamExt};

use futures_timer::Delay;

use tokio::{io, net::TcpStream, sync::mpsc};

use tokio_util::codec::{BytesCodec, Framed};

use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode},
    execute, style, terminal,
    terminal::ClearType,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MenuInput {
    Up,
    Down,
    Enter,
    Quit,
}

impl MenuInput {
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
    read_ctx: Option<crdts::ctx::ReadCtx<(), DocActor>>,
}

impl ClientState {
    fn new() -> Self {
        ClientState {
            actor: None,
            document: None,
            read_ctx: None,
        }
    }
}

#[derive(Debug)]
struct MenuState {
    pub index: usize,
    items: Vec<String>,
    pub menu_cmd_rx: mpsc::Receiver<MenuInput>,
    menu_cmd_tx: mpsc::Sender<MenuInput>,
}

impl MenuState {
    fn clone_menu_tx_channel(&self) -> mpsc::Sender<MenuInput> {
        self.menu_cmd_tx.clone()
    }

    fn command_menu() -> Self {
        let (menu_cmd_tx, menu_cmd_rx) = mpsc::channel(100);
        let items = vec![
            "Get document".into(),
            "Get record by key".into(),
            "Get document read context".into(),
            "Add a record".into(),
            "Apply an op".into(),
        ];
        MenuState {
            index: 0,
            items,
            menu_cmd_rx,
            menu_cmd_tx,
        }
    }

    fn choice_to_command(&self) -> Option<Command> {
        match self.index {
            0 => Some(Command::GetDocument),
            1 => Some(Command::GetRecord { key: 0 }),
            2 => Some(Command::GetReadCtx),
            3 => None,
            4 => None,
            // 3 => Command::,
            // 4 => Command::,
            _ => None,
        }
    }

    fn print_menu<W: Write>(&self, write: &mut W) -> crossterm::Result<()> {
        execute!(write, cursor::SavePosition, terminal::Clear(ClearType::All),)?;
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

    fn apply_command(&mut self, cmd: MenuInput) {
        match cmd {
            MenuInput::Up => self.previous(),
            MenuInput::Down => self.next(),
            MenuInput::Enter => (),
            _ => (),
        }
    }
}

async fn events_loop(
    tx: mpsc::Sender<MenuInput>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = EventStream::new();
    let mut tx: mpsc::Sender<MenuInput> = tx;

    loop {
        let mut event = reader.next().fuse();

        futures::select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {

                        if let Some(cmd) = MenuInput::from_event(&event) {
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

fn print_at<W: Write>(
    x: u16,
    y: u16,
    s: &str,
    write: &mut W,
) -> crossterm::Result<()> {
    execute!(
        write,
        cursor::SavePosition,
        cursor::MoveTo(x, y),
        terminal::Clear(terminal::ClearType::CurrentLine)
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
    let addr = "127.0.0.1:8080".parse::<SocketAddr>()?;
    let stream = TcpStream::connect(addr).await?;

    let framed = Framed::new(stream, BytesCodec::new());
    let (sink, mut stream) = framed.split();

    let (doc_cmd_tx, doc_cmd_rx) = mpsc::channel(100);

    let _recv_handle = tokio::spawn(async move {
        let mut stdout = stdout();
        while let Some(result) = stream.next().await {
            if let Ok(input) = result {
                if let Some(doc_resp) = DocResponse::from_bytes(&input) {
                    match doc_resp {
                        DocResponse::Document(doc) => {
                            print_at(5, 5, "Received doc", &mut stdout)
                                .unwrap();
                            for (i, item_ctx) in doc.records.iter().enumerate()
                            {
                                let _ = print_at(5, 6, "Doc:", &mut stdout);
                                let i = (7 + i) as u16;
                                let (k, v) = item_ctx.val;
                                let mut s = String::new();
                                v.read().val.iter().for_each(|x| {
                                    let string =
                                        std::str::from_utf8(x).unwrap();
                                    s.push_str(&format!(", {}", string));
                                });
                                let _ = print_at(
                                    5,
                                    i,
                                    &format!("{} - {}", k, s),
                                    &mut stdout,
                                );
                            }
                        }
                        DocResponse::Record(rec) => {
                            let rec = rec.val;
                            if let Some(record) = rec {
                                print_at(
                                    5,
                                    5,
                                    "Received filled record",
                                    &mut stdout,
                                )
                                .unwrap();
                                let mut rec_string = String::new();
                                record.read().val.iter().for_each(|x| {
                                    let s = std::str::from_utf8(x).unwrap();
                                    rec_string.push_str(&format!(" {}", s));
                                });
                                let fmted = format!("Record:\n{}", rec_string);
                                let _ = print_at(5, 6, &fmted, &mut stdout);
                            } else {
                                print_at(
                                    5,
                                    5,
                                    "Received empty record",
                                    &mut stdout,
                                )
                                .unwrap();
                            }
                        }
                        DocResponse::ReadCtx(_ctx) => {
                            print_at(5, 5, "Received read ctx", &mut stdout)
                                .unwrap();
                        }
                    }
                }
            }
        }
    });

    let _send_handle = tokio::spawn(async move {
        let _ = send_cmds_handler(doc_cmd_rx, sink).await;
    });

    let menu_state = MenuState::command_menu();

    let mut sout = stdout();

    execute!(
        sout,
        terminal::Clear(ClearType::All),
        cursor::DisableBlinking,
        cursor::Hide,
        cursor::MoveTo(0, 0),
    )?;

    terminal::enable_raw_mode()?;

    let menu_tx = menu_state.clone_menu_tx_channel();

    let _ev_loop_handle = tokio::spawn(async move {
        let _ = events_loop(menu_tx).await;
    });

    menu_state.print_menu(&mut sout)?;

    menu_handler(menu_state, doc_cmd_tx, stdout()).await?;

    terminal::disable_raw_mode()?;

    execute!(sout, cursor::EnableBlinking, cursor::Show)?;
    Ok(())
}

async fn menu_handler<W: Write>(
    mut menu_state: MenuState,
    mut doc_cmd_tx: mpsc::Sender<Command>,
    mut write: W,
) -> Result<(), Box<dyn std::error::Error>> {
    while let Some(cmd) = menu_state.menu_cmd_rx.recv().await {
        match cmd {
            MenuInput::Quit => break,
            MenuInput::Enter => {
                if let Some(doc_cmd) = menu_state.choice_to_command() {
                    doc_cmd_tx.send(doc_cmd).await?;
                }
            }
            _ => {
                menu_state.apply_command(cmd);
                menu_state.print_menu(&mut write)?;
            }
        }
    }
    Ok(())
}

async fn send_cmds_handler(
    mut doc_cmd_rx: mpsc::Receiver<Command>,
    mut sink: impl Sink<Bytes, Error = io::Error> + Unpin,
) -> Result<(), Box<dyn std::error::Error>> {
    while let Some(cmd) = doc_cmd_rx.recv().await {
        let bytes = bincode::serialize(&cmd)?;
        sink.send(Bytes::from(bytes)).await?;
        sink.flush().await?;
    }

    Ok(())
}
