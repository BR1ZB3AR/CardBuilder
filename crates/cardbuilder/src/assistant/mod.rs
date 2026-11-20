//! The Assistant: AI agents that design with you. Agents connect over MCP, the Model Context
//! Protocol (`mcp.rs`): CardBuilder runs a small server on this computer (127.0.0.1 only) and
//! offers tools to read, change, render and export the design (`tools.rs`). Every call runs on
//! the UI thread as an ordinary edit, so Ctrl+Z undoes it; what an agent may do without asking
//! is set in Preferences › Assistant (`ui.rs` draws that page, the approval card, the agent's
//! pointer and the Assistant button's panel).
//!
//! The web version has only the settings type: a browser can't listen on a port.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
mod mcp;
#[cfg(not(target_arch = "wasm32"))]
mod tools;
#[cfg(not(target_arch = "wasm32"))]
mod ui;

/// the port agents connect to unless the user picks another
pub const DEFAULT_PORT: u16 = 27879;

/// how much an agent may do without asking
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    /// every change asks first
    AskFirst,
    /// changes to the design go ahead (Ctrl+Z undoes them); opening, saving and exporting ask
    #[default]
    MakeEdits,
    /// nothing asks
    DoEverything,
}

/// Preferences › Assistant (kept with the app's other preferences)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssistantPrefs {
    /// every AI feature at all (off: no Assistant button, no pointer, no connection)
    pub show_ai: bool,
    /// the Assistant button in the top bar
    pub app_bar_button: bool,
    pub permission: Permission,
    /// tools allowed without asking, whatever the permission (Always allow)
    pub always_allowed: Vec<String>,
    /// the user's own instructions, sent to agents when they connect
    pub instructions: String,
    /// an animated pointer on the card shows what the agent is working on
    pub pointer: bool,
    /// how fast the pointer moves (1 = normal)
    pub pointer_speed: f32,
    /// select the layer the agent changes, so its settings show in the sidebar
    pub follow: bool,
    /// the MCP server: agents on this computer may connect
    pub mcp: bool,
    pub port: u16,
    /// shorter tool results (no indentation, fewer details)
    pub compact: bool,
}

impl Default for AssistantPrefs {
    fn default() -> Self {
        AssistantPrefs {
            show_ai: true,
            app_bar_button: true,
            permission: Permission::MakeEdits,
            always_allowed: Vec::new(),
            instructions: String::new(),
            pointer: true,
            pointer_speed: 1.0,
            follow: false,
            // listening on a port is the user's choice: one click in the Assistant panel
            mcp: false,
            port: DEFAULT_PORT,
            compact: false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::mcp::{Call, Outcome, Server, Shared};
    use super::tools::{self, Kind};
    use super::AssistantPrefs;
    use crate::app::CardBuilderApp;
    use cb_core::doc::Side;
    use std::collections::VecDeque;
    use std::sync::mpsc::Receiver;
    use std::sync::Arc;

    /// what the server is doing
    #[derive(Clone, Debug, PartialEq)]
    pub enum Status {
        Off,
        Listening(u16),
        Failed(u16, String),
    }

    /// one thing an agent did (or tried), for the Assistant panel
    #[derive(Clone, Debug)]
    pub struct Activity {
        pub time: f64,
        pub agent: String,
        pub what: String,
        pub result: ActResult,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ActResult {
        Done,
        Denied,
        Failed,
    }

    /// a call waiting for the user's answer
    pub struct Pending {
        pub call: Call,
        pub what: String,
        pub since: f64,
    }

    /// the agent's pointer on the card: where it is heading, from where, since when
    #[derive(Clone, Debug, Default)]
    pub struct Pointer {
        pub target: Option<(Side, f64, f64)>,
        pub from: Option<egui::Pos2>,
        pub t0: f64,
        pub label: String,
        /// last frame's drawn position (where the next move starts)
        pub at: Option<egui::Pos2>,
    }

    /// the user's answer to an approval card
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Answer {
        Allow,
        Always,
        Deny,
    }

    /// seconds an approval card waits before it counts as Deny
    pub const ASK_TIMEOUT: f64 = 120.0;

    pub struct Assistant {
        server: Option<Server>,
        calls: Option<Receiver<Call>>,
        pub status: Status,
        pub shared: Arc<Shared>,
        /// (server wanted, port) last applied
        applied: Option<(bool, u16)>,
        /// instructions last handed to the server
        applied_instructions: Option<String>,
        pub log: VecDeque<Activity>,
        pub pending: Option<Pending>,
        pub pointer: Pointer,
        /// the Assistant button's panel is open
        pub panel: bool,
        ctx: egui::Context,
    }

    impl Assistant {
        pub fn new(ctx: &egui::Context) -> Assistant {
            Assistant {
                server: None,
                calls: None,
                status: Status::Off,
                shared: Arc::new(Shared::default()),
                applied: None,
                applied_instructions: None,
                log: VecDeque::new(),
                pending: None,
                pointer: Pointer::default(),
                panel: false,
                ctx: ctx.clone(),
            }
        }

        /// start, stop or move the server to match the preferences
        pub fn sync(&mut self, p: &AssistantPrefs) {
            let want = (p.show_ai && p.mcp, p.port);
            if self.applied != Some(want) {
                self.applied = Some(want);
                self.server = None; // stops the old one (Drop)
                self.calls = None;
                self.status = Status::Off;
                if want.0 {
                    match Server::start(p.port, self.shared.clone(), self.ctx.clone()) {
                        Ok((s, rx)) => {
                            self.server = Some(s);
                            self.calls = Some(rx);
                            self.status = Status::Listening(p.port);
                        }
                        Err(e) => self.status = Status::Failed(p.port, e),
                    }
                }
            }
            let text = tools::instructions(&p.instructions);
            if self.applied_instructions.as_deref() != Some(text.as_str()) {
                *self.shared.instructions.lock().unwrap() = text.clone();
                self.applied_instructions = Some(text);
            }
        }

        fn note(&mut self, time: f64, agent: &str, what: String, result: ActResult) {
            self.log.push_front(Activity { time, agent: agent.to_string(), what, result });
            self.log.truncate(60);
        }

        /// agents connected now (seen in the last ten minutes)
        pub fn sessions(&self) -> Vec<super::mcp::Session> {
            let s = self.shared.sessions.lock().unwrap();
            s.iter().filter(|x| x.last.elapsed().as_secs() < 600).cloned().collect()
        }
    }

    impl CardBuilderApp {
        /// once a frame: keep the server in step with the preferences and run the agents' calls
        pub(crate) fn assistant_frame(&mut self, ctx: &egui::Context, now: f64) {
            let prefs = self.prefs.assistant.clone();
            self.assistant.sync(&prefs);
            if let Some(p) = &self.assistant.pending {
                if now - p.since > ASK_TIMEOUT {
                    self.answer(Answer::Deny, now);
                }
            }
            // one approval at a time: later calls wait in the channel
            while self.assistant.pending.is_none() {
                let Some(call) = self.assistant.calls.as_ref().and_then(|rx| rx.try_recv().ok()) else { break };
                let kind = tools::kind(&call.tool).unwrap_or(Kind::Edit);
                let what = self.describe_call(&call.tool, &call.args);
                if needs_asking(&prefs, kind, &call.tool) {
                    self.assistant.pending = Some(Pending { call, what, since: now });
                    ctx.request_repaint();
                } else {
                    self.run_call(call, what, now);
                }
            }
            if self.assistant.pending.is_some() {
                // the card's countdown
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }

        /// the user's answer to the waiting call
        pub(crate) fn answer(&mut self, a: Answer, now: f64) {
            let Some(p) = self.assistant.pending.take() else { return };
            match a {
                Answer::Deny => {
                    let msg = if now - p.since > ASK_TIMEOUT { "The user didn't answer within two minutes, so this was not done." } else { "The user said no to this. Don't try it again unless they ask." };
                    let _ = p.call.reply.send(Outcome::error(msg));
                    self.assistant.note(now, &p.call.agent, p.what, ActResult::Denied);
                }
                Answer::Always | Answer::Allow => {
                    if a == Answer::Always && !self.prefs.assistant.always_allowed.contains(&p.call.tool) {
                        self.prefs.assistant.always_allowed.push(p.call.tool.clone());
                    }
                    self.run_call(p.call, p.what, now);
                }
            }
        }

        fn run_call(&mut self, call: Call, what: String, now: f64) {
            let compact = self.prefs.assistant.compact;
            let out = self.run_tool(&call.tool, &call.args, now, compact);
            let result = if out.is_error { ActResult::Failed } else { ActResult::Done };
            if let Some((side, x, y, id)) = &out.focus {
                if self.prefs.assistant.pointer {
                    let p = &mut self.assistant.pointer;
                    p.from = p.at;
                    p.target = Some((*side, *x, *y));
                    p.t0 = now;
                    p.label = call.agent.clone();
                }
                if self.prefs.assistant.follow {
                    if let Some(id) = id {
                        self.selected = Some(id.clone());
                    }
                    self.side = *side;
                }
            }
            let _ = call.reply.send(out.into_outcome());
            self.assistant.note(now, &call.agent, what, result);
        }
    }

    /// does this call wait for the user?
    pub fn needs_asking(p: &AssistantPrefs, kind: Kind, tool: &str) -> bool {
        if kind == Kind::Read || p.always_allowed.iter().any(|t| t == tool) {
            return false;
        }
        match p.permission {
            super::Permission::AskFirst => true,
            super::Permission::MakeEdits => kind == Kind::File,
            super::Permission::DoEverything => false,
        }
    }

}

/// the web version: no agents (a browser can't listen on a port)
#[cfg(target_arch = "wasm32")]
pub struct Assistant;

#[cfg(target_arch = "wasm32")]
impl Assistant {
    pub fn new(_ctx: &egui::Context) -> Assistant {
        Assistant
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::tools::Kind;
    use super::*;

    #[test]
    fn permissions_decide_what_asks() {
        let mut p = AssistantPrefs::default();
        // reading never asks; Make edits asks for files only
        assert!(!needs_asking(&p, Kind::Read, "render"));
        assert!(!needs_asking(&p, Kind::Edit, "add_layer"));
        assert!(needs_asking(&p, Kind::File, "save_design"));
        p.permission = Permission::AskFirst;
        assert!(needs_asking(&p, Kind::Edit, "add_layer"));
        assert!(!needs_asking(&p, Kind::Read, "get_design"));
        // Always allow beats Ask first
        p.always_allowed.push("add_layer".into());
        assert!(!needs_asking(&p, Kind::Edit, "add_layer"));
        p.permission = Permission::DoEverything;
        assert!(!needs_asking(&p, Kind::File, "export"));
    }
}
