//! The MCP server agents connect to: JSON-RPC over HTTP (MCP's "Streamable HTTP" transport,
//! answering with plain JSON) at http://127.0.0.1:<port>/mcp. It listens on this computer only
//! and refuses requests from web pages (an `Origin` that isn't localhost), so a website can't
//! reach it. Tool calls are handed to the UI thread (`Call`) and answered from there.

use super::tools;
use serde_json::{json, Value};
use std::io::Read as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// MCP versions this server speaks, newest first
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// a tool call may wait this long for the user's answer (the approval card gives up at two
/// minutes) and for big exports
const CALL_TIMEOUT: Duration = Duration::from_secs(600);
/// biggest request accepted (a picture sent as a data URL)
const MAX_BODY: u64 = 64 << 20;

/// a tool call for the UI thread
pub struct Call {
    pub agent: String,
    pub tool: String,
    pub args: Value,
    pub reply: SyncSender<Outcome>,
}

/// a tool's answer: MCP content items, and whether it failed
pub struct Outcome {
    pub content: Vec<Value>,
    pub is_error: bool,
}

impl Outcome {
    pub fn error(msg: &str) -> Outcome {
        Outcome { content: vec![json!({"type": "text", "text": msg})], is_error: true }
    }
}

/// an agent that connected
#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    /// its name as people know it ("Claude Code")
    pub agent: String,
    pub version: String,
    pub since: Instant,
    pub last: Instant,
    pub calls: u32,
}

/// what the server threads and the app share
#[derive(Default)]
pub struct Shared {
    pub sessions: Mutex<Vec<Session>>,
    /// sent to agents when they connect
    pub instructions: Mutex<String>,
}

/// the running server; dropping it stops it
pub struct Server {
    http: Arc<tiny_http::Server>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(port: u16, shared: Arc<Shared>, ctx: egui::Context) -> Result<(Server, Receiver<Call>), String> {
        let http = tiny_http::Server::http(("127.0.0.1", port)).map_err(|e| {
            let e = e.to_string();
            if e.contains("in use") { format!("port {port} is in use by another program") } else { e }
        })?;
        let http = Arc::new(http);
        let (tx, rx) = mpsc::channel();
        let h = http.clone();
        let thread = std::thread::Builder::new()
            .name("assistant-mcp".into())
            .spawn(move || {
                for req in h.incoming_requests() {
                    let (tx, shared, ctx) = (tx.clone(), shared.clone(), ctx.clone());
                    // a call can wait for the user: each request gets its own thread
                    let _ = std::thread::Builder::new().name("assistant-request".into()).spawn(move || handle(req, &tx, &shared, &ctx));
                }
            })
            .map_err(|e| e.to_string())?;
        log::info!("Assistant: MCP server on http://127.0.0.1:{port}/mcp");
        Ok((Server { http, thread: Some(thread) }, rx))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.http.unblock();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn header(req: &tiny_http::Request, name: &str) -> Option<String> {
    req.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h| h.value.as_str().to_string())
}

/// a page on this computer (or no page at all) may call; any other website may not
fn local_origin(origin: &str) -> bool {
    let rest = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")).unwrap_or("");
    let host = rest.split(['/', ':']).next().unwrap_or("");
    matches!(host, "127.0.0.1" | "localhost" | "[::1]") || origin == "null"
}

fn respond(req: tiny_http::Request, code: u16, body: Option<&Value>, session: Option<&str>) {
    let text = body.map(|b| b.to_string()).unwrap_or_default();
    let mut r = tiny_http::Response::from_string(text).with_status_code(code);
    if body.is_some() {
        r.add_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap());
    }
    if let Some(s) = session {
        r.add_header(tiny_http::Header::from_bytes("Mcp-Session-Id", s).unwrap());
    }
    let _ = req.respond(r);
}

fn handle(mut req: tiny_http::Request, tx: &Sender<Call>, shared: &Shared, ctx: &egui::Context) {
    if let Some(o) = header(&req, "Origin") {
        if !local_origin(&o) {
            return respond(req, 403, Some(&json!({"error": "CardBuilder only takes calls from this computer"})), None);
        }
    }
    let path = req.url().split('?').next().unwrap_or("").trim_end_matches('/').to_string();
    if path != "/mcp" && !path.is_empty() {
        return respond(req, 404, Some(&json!({"error": "CardBuilder's MCP endpoint is /mcp"})), None);
    }
    let session = header(&req, "Mcp-Session-Id");
    match req.method() {
        tiny_http::Method::Post => {}
        tiny_http::Method::Delete => {
            if let Some(id) = &session {
                shared.sessions.lock().unwrap().retain(|s| &s.id != id);
            }
            return respond(req, 200, None, None);
        }
        // no stream of server messages: everything comes back as the answer to a POST
        _ => {
            let r = tiny_http::Response::from_string("").with_status_code(405).with_header(tiny_http::Header::from_bytes("Allow", "POST, DELETE").unwrap());
            let _ = req.respond(r);
            return;
        }
    }
    let mut body = String::new();
    if req.as_reader().take(MAX_BODY).read_to_string(&mut body).is_err() {
        return respond(req, 400, Some(&rpc_error(Value::Null, -32700, "the request isn't UTF-8 text")), None);
    }
    let msg: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return respond(req, 400, Some(&rpc_error(Value::Null, -32700, &format!("not JSON: {e}"))), None),
    };
    let mut new_session = None;
    let out = match msg {
        Value::Array(items) => {
            let answers: Vec<Value> = items.iter().filter_map(|m| one(m, session.as_deref(), tx, shared, ctx, &mut new_session)).collect();
            (!answers.is_empty()).then_some(Value::Array(answers))
        }
        m => one(&m, session.as_deref(), tx, shared, ctx, &mut new_session),
    };
    match out {
        Some(v) => respond(req, 200, Some(&v), new_session.as_deref()),
        // only notifications: accepted, nothing to answer
        None => respond(req, 202, None, None),
    }
}

fn rpc_error(id: Value, code: i64, msg: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg}})
}

/// "claude-code" → "Claude Code"
fn agent_name(client: &str) -> String {
    match client.to_ascii_lowercase().as_str() {
        "claude-code" | "claude code" => "Claude Code".into(),
        "gemini-cli" | "gemini-cli-mcp-client" => "Gemini CLI".into(),
        "codex" | "codex-mcp-client" => "Codex".into(),
        "" => "An agent".into(),
        _ => client
            .split(['-', '_'])
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn new_session_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("cb-{:x}-{:x}", t, N.fetch_add(1, Ordering::Relaxed))
}

/// answer one JSON-RPC message (None for a notification)
fn one(msg: &Value, session: Option<&str>, tx: &Sender<Call>, shared: &Shared, ctx: &egui::Context, new_session: &mut Option<String>) -> Option<Value> {
    let id = msg.get("id").cloned()?;
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    let result: Result<Value, (i64, String)> = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(VERSIONS[0]);
            let version = VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(VERSIONS[0]);
            let client = params.pointer("/clientInfo/name").and_then(Value::as_str).unwrap_or("");
            let sid = new_session_id();
            let now = Instant::now();
            shared.sessions.lock().unwrap().push(Session {
                id: sid.clone(),
                agent: agent_name(client),
                version: params.pointer("/clientInfo/version").and_then(Value::as_str).unwrap_or("").to_string(),
                since: now,
                last: now,
                calls: 0,
            });
            *new_session = Some(sid);
            ctx.request_repaint();
            Ok(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "cardbuilder", "title": "CardBuilder", "version": env!("CARGO_PKG_VERSION")},
                "instructions": shared.instructions.lock().unwrap().clone(),
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools::list()})),
        "resources/list" => Ok(json!({"resources": []})),
        "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
        "prompts/list" => Ok(json!({"prompts": []})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if tools::kind(&name).is_none() {
                Ok(json!({"content": [{"type": "text", "text": format!("CardBuilder has no tool called {name}")}], "isError": true}))
            } else {
                let agent = {
                    let mut s = shared.sessions.lock().unwrap();
                    match s.iter_mut().find(|x| Some(x.id.as_str()) == session) {
                        Some(x) => {
                            x.last = Instant::now();
                            x.calls += 1;
                            x.agent.clone()
                        }
                        None => "An agent".to_string(),
                    }
                };
                let (rtx, rrx) = mpsc::sync_channel(1);
                if tx.send(Call { agent, tool: name, args, reply: rtx }).is_err() {
                    Err((-32603, "CardBuilder is closing".into()))
                } else {
                    ctx.request_repaint();
                    match rrx.recv_timeout(CALL_TIMEOUT) {
                        Ok(o) => Ok(json!({"content": o.content, "isError": o.is_error})),
                        Err(_) => Ok(json!({"content": [{"type": "text", "text": "CardBuilder didn't answer in time"}], "isError": true})),
                    }
                }
            }
        }
        _ => Err((-32601, format!("CardBuilder doesn't know the method {method}"))),
    };
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, m)) => rpc_error(id, code, &m),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_computer_may_call() {
        assert!(local_origin("http://localhost:5173"));
        assert!(local_origin("http://127.0.0.1"));
        assert!(!local_origin("https://example.com"));
        assert!(!local_origin("http://localhost.evil.com"));
        assert_eq!(agent_name("claude-code"), "Claude Code");
        assert_eq!(agent_name("my-cool_agent"), "My Cool Agent");
    }

    /// the whole conversation over real HTTP: connect, list the tools, call one; the UI side
    /// is played by this test
    #[test]
    fn an_agent_connects_and_calls_a_tool() {
        let shared = Arc::new(Shared::default());
        *shared.instructions.lock().unwrap() = "be kind".into();
        let port = 27000 + (std::process::id() % 2000) as u16;
        let (server, rx) = Server::start(port, shared.clone(), egui::Context::default()).expect("server");
        let ui = std::thread::spawn(move || {
            let call = rx.recv_timeout(Duration::from_secs(10)).expect("a call");
            assert_eq!((call.agent.as_str(), call.tool.as_str()), ("Claude Code", "get_design"));
            let _ = call.reply.send(Outcome { content: vec![json!({"type": "text", "text": "{}"})], is_error: false });
        });
        let post = |body: Value, sid: Option<&str>| -> (u16, Option<String>, String) {
            use std::io::Write as _;
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            let b = body.to_string();
            let extra = sid.map(|x| format!("Mcp-Session-Id: {x}\r\n")).unwrap_or_default();
            write!(s, "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len()).unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).unwrap();
            let code = out[9..12].parse().unwrap();
            let sid = out.lines().find_map(|l| l.strip_prefix("Mcp-Session-Id: ").map(str::to_string));
            (code, sid, out.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
        };
        let (code, sid, body) = post(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "clientInfo": {"name": "claude-code", "version": "2.0"}, "capabilities": {}}}), None);
        assert_eq!(code, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["result"]["instructions"], "be kind");
        let sid = sid.expect("a session id");
        assert_eq!(post(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}), Some(&sid)).0, 202);
        let (_, _, body) = post(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}), Some(&sid));
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "add_layer"));
        let (_, _, body) = post(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_design", "arguments": {}}}), Some(&sid));
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["result"]["isError"], false);
        ui.join().unwrap();
        assert_eq!(shared.sessions.lock().unwrap()[0].calls, 1);
        drop(server);
    }
}
