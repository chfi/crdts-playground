import * as wasm from "wasm-client";
import { memory } from "wasm-client/wasm_client_bg";

const ws_conn = wasm.WSConnection.new("ws://127.0.0.1:3030/echo");

window.ws_conn = ws_conn;
