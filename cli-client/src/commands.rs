use crdts_sandbox_lib::document::{
    Command, DocActor, DocResponse, Document, DocumentOp, RecordEntry,
};

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

pub struct GetDocument();

pub struct GetRecord {
    key: u64,
}

pub struct GetReadCtx();

pub struct AddRecordOp {
    add_ctx: AddCtx<DocActor>,
    key: RecordKey,
    content: String,
}

pub struct ApplyLocalOp {
    op: DocumentOp,
}

pub struct ApplyRemoteOp {
    op: DocumentOp,
}
