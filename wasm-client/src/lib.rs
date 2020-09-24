mod utils;

use futures::{channel::mpsc, future::FutureExt, Stream, StreamExt};

use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use crdts_sandbox_lib::document::{
    Command, DocActor, DocResponse, Document, DocumentOp,
};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[allow(dead_code)]
#[wasm_bindgen]
pub struct WSConnection {
    ws: WebSocket,
    onopen: Closure<dyn FnMut(JsValue)>,
    onmessage: Closure<dyn FnMut(MessageEvent)>,
    onerror: Closure<dyn FnMut(ErrorEvent)>,
    receiver: mpsc::Receiver<MessageEvent>,
}

#[wasm_bindgen]
impl WSConnection {
    async fn get_received(&mut self) -> Option<MessageEvent> {
        self.receiver.next().await
    }

    pub fn get_message(&mut self) -> Option<MessageEvent> {
        if let Ok(Some(e)) = self.receiver.try_next() {
            Some(e)
        } else {
            None
        }
    }

    fn get_docresp(&mut self) -> Option<DocResponse> {
        let msg = self.get_message()?;

        if let Ok(abuf) = msg.data().dyn_into::<js_sys::ArrayBuffer>() {
            let array = js_sys::Uint8Array::new(&abuf);
            // let len = array.byte_length() as usize;
            let bytes = array.to_vec();
            DocResponse::from_bytes(&bytes)
        } else {
            None
        }
    }

    pub fn print_docresp(&mut self) {
        if let Some(resp) = self.get_docresp() {
            let string = match resp {
                DocResponse::Document(doc) => {
                    console_log!("received document");
                    for item_ctx in doc.records.iter() {
                        let (k, v) = item_ctx.val;
                        console_log!("item {}", k);
                        v.read().val.iter().for_each(|x| {
                            let s = std::str::from_utf8(x).unwrap();
                            console_log!("  {}", s);
                        });
                    }
                }
                DocResponse::Record(rec) => {
                    let rec = rec.val;
                    if let Some(record) = rec {
                        console_log!("received record");
                        record.read().val.iter().for_each(|x| {
                            let s = std::str::from_utf8(x).unwrap();
                            console_log!("  {}", s);
                        });
                    }
                }
                DocResponse::ReadCtx(read_ctx) => {
                    console_log!("received readctx");
                }
            };
        } else {
            console_log!("no docresp available");
        }
    }

    pub fn send_str(&self, data: &str) -> Result<(), JsValue> {
        self.ws.send_with_str(data)
    }

    fn send_command(&self, cmd: Command) -> Result<(), JsValue> {
        let message = cmd.to_bytes().unwrap();
        self.ws.send_with_u8_array(&message)
    }

    pub fn send_get_document(&self) -> Result<(), JsValue> {
        self.send_command(Command::GetDocument)
    }

    pub fn send_get_record(&self, key: u32) -> Result<(), JsValue> {
        self.send_command(Command::GetRecord { key })
    }

    pub fn send_get_read_ctx(&self) -> Result<(), JsValue> {
        self.send_command(Command::GetReadCtx)
    }

    pub fn print_received_message(&mut self) {
        if let Ok(msg) = self.receiver.try_next() {
            if let Some(e) = msg {
                if let Ok(abuf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                    console_log!(
                        "message event, received arraybuffer: {:?}",
                        abuf
                    );
                    let array = js_sys::Uint8Array::new(&abuf);
                    let len = array.byte_length() as usize;
                    console_log!(
                        "Arraybuffer received {}bytes: {:?}",
                        len,
                        array.to_vec()
                    );
                } else if let Ok(blob) = e.data().dyn_into::<web_sys::Blob>() {
                    console_log!("message event, received blob: {:?}", blob);
                } else if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>()
                {
                    console_log!("message event, received Text: {:?}", txt);
                } else {
                    console_log!(
                        "message event, received Unknown: {:?}",
                        e.data()
                    );
                }
            }
        }
    }

    pub fn new(url: &str) -> Result<WSConnection, JsValue> {
        let ws = WebSocket::new(url)?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let (sender, receiver) = mpsc::channel(64);

        // onmessage callback
        // let cloned_ws = ws.clone();
        let mut cloned_sender = sender.clone();
        let onmessage_callback =
            Closure::wrap(Box::new(move |e: MessageEvent| {
                match cloned_sender.try_send(e) {
                    Ok(()) => console_log!("passed on message"),
                    Err(_) => console_log!("Couldn't send message on channel"),
                };
            }) as Box<dyn FnMut(MessageEvent)>);

        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));

        let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
            console_log!("error event: {:?}", e);
        })
            as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));

        let cloned_ws = ws.clone();
        let onopen_callback = Closure::wrap(Box::new(move |_| {
            console_log!("socket opened");
            match cloned_ws.send_with_str("ping") {
                Ok(_) => console_log!("message successfully sent"),
                Err(err) => console_log!("error sending message: {:?}", err),
            }
            // send off binary message
            match cloned_ws.send_with_u8_array(&vec![0, 1, 2, 3]) {
                Ok(_) => console_log!("binary message successfully sent"),
                Err(err) => console_log!("error sending message: {:?}", err),
            }
        })
            as Box<dyn FnMut(JsValue)>);

        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));

        Ok(WSConnection {
            ws,
            onopen: onopen_callback,
            onmessage: onmessage_callback,
            onerror: onerror_callback,
            receiver,
        })
    }
}
