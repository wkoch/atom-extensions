//! `atom/ui-web`: shell web mínimo (ADR-016, v0.8).
//!
//! - Serve HTTP em 127.0.0.1:0: shell estático (`assets/`) + `/api/*`.
//! - `/api/mods` = pacotes `kind="ui-mod"` + bindings (`atom.packages.mods`).
//! - `/api/session` = `{wsUrl, token}` (`atom.ui.session`; null sem `--ws`).
//! - `/mod/<pub>/<nome>/<arq>` = asset do mod (`atom.packages.asset`).
//! - Anuncia a base em `atom.ui.serve` (aparece no `status`).
//! - Sem telas próprias: tudo o que aparece vem de mods via contratos.
//!
//! Concorrência: o SDK dirige o bus (`run`); threads HTTP chamam o core via
//! `Client` (`Clone + Send`) com `Handle::block_on`. Uma chamada bus por vez
//! continua valendo no lado do core; cada pedido HTTP bloqueia sua thread.
//!
//! Sem `window`/`ack`/`gap`: o SDK fixado não despacha `command.run` e este
//! shell não usa streams.

use atom_sdk::{Client, Declare, Handler};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Mutex;

struct H {
    state: Mutex<State>,
}

struct State {
    /// Base HTTP anunciada; `Some` = listener ligado + `atom.ui.serve` enviado.
    url: Option<String>,
    pkg_dir: String,
}

fn content_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

fn reply_http(stream: &mut std::net::TcpStream, code: &str, ctype: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {code}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn clean_rel(parts: &[&str]) -> Option<String> {
    let mut out = vec![];
    for p in parts {
        if p.is_empty() || *p == "." || *p == ".." {
            return None;
        }
        if p.contains('\\') || p.contains('\0') {
            return None;
        }
        out.push(*p);
    }
    if out.is_empty() {
        return None;
    }
    Some(out.join("/"))
}

/// Chamada ao core a partir de thread HTTP (fora do runtime): bloqueia a
/// thread até a resposta. Erro vira a mensagem (fio: `error.message`).
fn bus_call(
    rt: &tokio::runtime::Handle,
    client: &Client,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    match rt.block_on(client.request(method, params)) {
        Ok(v) => Ok(v),
        Err(e) => Err(e.message),
    }
}

fn handle_http(
    mut stream: std::net::TcpStream,
    pkg_dir: String,
    client: Client,
    rt: tokio::runtime::Handle,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut head = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line == "\r\n" || line == "\n" || line.trim().is_empty() {
            break;
        }
        head.push_str(&line);
        if head.len() > 65536 {
            return;
        }
    }
    let first = head.lines().next().unwrap_or("");
    let mut it = first.split_whitespace();
    let method = it.next().unwrap_or("");
    let path = it.next().unwrap_or("/");
    if method != "GET" {
        reply_http(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"GET apenas",
        );
        return;
    }
    let path = path.split('?').next().unwrap_or("/");
    // Arquivos estáticos do shell.
    for name in ["index.html", "shell.js", "style.css"] {
        if path == format!("/{name}") || (name == "index.html" && path == "/") {
            let f = format!("{pkg_dir}/assets/{name}");
            match std::fs::read(&f) {
                Ok(b) => reply_http(&mut stream, "200 OK", content_type(name), &b),
                Err(_) => reply_http(&mut stream, "404 Not Found", "text/plain", b"ausente"),
            }
            return;
        }
    }
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let mut answer = |v: Result<Value, String>| match v {
        Ok(body) => {
            let s = serde_json::to_string(&body).unwrap_or_default();
            reply_http(&mut stream, "200 OK", "application/json", s.as_bytes());
        }
        Err(e) => reply_http(&mut stream, "502 Bad Gateway", "text/plain", e.as_bytes()),
    };
    match segs.as_slice() {
        ["api", "mods"] => {
            answer(bus_call(&rt, &client, "atom.packages.mods", json!({})));
        }
        ["api", "session"] => {
            answer(bus_call(&rt, &client, "atom.ui.session", json!({})));
        }
        ["mod", pub_, name, rest @ ..] => {
            let pkg = format!("{pub_}/{name}");
            match clean_rel(rest) {
                Some(rel) => {
                    let ct = content_type(&rel);
                    match bus_call(
                        &rt,
                        &client,
                        "atom.packages.asset",
                        json!({"package": pkg, "path": rel}),
                    ) {
                        Ok(v) => {
                            let data = v.get("data").and_then(|d| d.as_str()).unwrap_or("");
                            let enc = v.get("encoding").and_then(|e| e.as_str()).unwrap_or("");
                            if enc != "base64" {
                                reply_http(
                                    &mut stream,
                                    "502 Bad Gateway",
                                    "text/plain",
                                    b"encoding?",
                                );
                                return;
                            }
                            // `shortcut: sem base64 no std; decodificação manual mínima.`
                            match b64_decode(data) {
                                Some(b) => reply_http(&mut stream, "200 OK", ct, &b),
                                None => reply_http(
                                    &mut stream,
                                    "502 Bad Gateway",
                                    "text/plain",
                                    b"base64?",
                                ),
                            }
                        }
                        Err(e) => {
                            reply_http(&mut stream, "404 Not Found", "text/plain", e.as_bytes())
                        }
                    }
                }
                None => reply_http(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain",
                    "caminho inválido".as_bytes(),
                ),
            }
        }
        _ => reply_http(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"rota desconhecida",
        ),
    }
}

/// Decodifica base64 padrão (ignora whitespace). Suficiente p/ assets.
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = s.bytes().filter(|b| !b" \t\r\n".contains(b)).collect();
    if !clean.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for q in clean.chunks(4) {
        let mut n = 0u32;
        let mut pad = 0;
        for (i, &c) in q.iter().enumerate() {
            if c == b'=' {
                pad += 1;
                n <<= 6;
                let _ = i;
            } else {
                n = (n << 6) | val(c)? as u32;
            }
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

impl Handler for H {
    /// O `initialize` manual anunciava `provides: []` (sem contratos, sem
    /// handles); `subscribes` espelha o manifesto (entrega via `event.deliver`).
    fn declare(&self) -> Declare {
        Declare {
            subscribes: vec!["atom.extension.started".to_string()],
            ..Default::default()
        }
    }

    /// Fim do boot: sobe o HTTP (porta efêmera em 127.0.0.1) e anuncia a base
    /// em `atom.ui.serve`. O SDK fixado não entrega `atom.initialized` ao
    /// handler; o evento de ciclo de vida do próprio pacote é o ponto
    /// equivalente (o core publica `atom.extension.started` após o handshake,
    /// com esta extensão já em execução).
    async fn on_event(&self, client: &Client, ev: atom_sdk::Event) {
        if ev.topic != "atom.extension.started" {
            return;
        }
        let pkg = ev
            .data
            .get("package")
            .and_then(|p| p.as_str())
            .unwrap_or("");
        let me = std::env::var("ATOM_EXTENSION")
            .ok()
            .and_then(|v| v.split('@').next().map(|s| s.to_string()));
        if me.as_deref() != Some(pkg) {
            return;
        }
        let url = {
            let mut st = self.state.lock().unwrap();
            if st.url.is_some() {
                return;
            }
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("ui-web bind: {e}");
                    return;
                }
            };
            let addr = match listener.local_addr() {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("ui-web addr: {e}");
                    return;
                }
            };
            let url = format!("http://{addr}");
            eprintln!("ui-web em {url}");
            let pkg = st.pkg_dir.clone();
            let c = client.clone();
            let rt = tokio::runtime::Handle::current();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let pkg = pkg.clone();
                    let c = c.clone();
                    let rt = rt.clone();
                    std::thread::spawn(move || handle_http(stream, pkg, c, rt));
                }
            });
            st.url = Some(url.clone());
            url
        };
        if let Err(e) = client.request("atom.ui.serve", json!({"url": url})).await {
            eprintln!("ui-web anuncia: {}", e.message);
        }
    }
}

#[tokio::main]
async fn main() {
    let pkg_dir = std::env::var("ATOM_PKG_DIR").unwrap_or_else(|_| ".".to_string());
    atom_sdk::run(H {
        state: Mutex::new(State { url: None, pkg_dir }),
    })
    .await;
}
