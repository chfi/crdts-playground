import * as wasm from "wasm-client";
import { memory } from "wasm-client/wasm_client_bg";

const ws_conn = wasm.WSConnection.new("wss://echo.websocket.org");

window.ws_conn = ws_conn;
