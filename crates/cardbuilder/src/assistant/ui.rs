//! What the Assistant shows: Preferences › Assistant, the approval card ("Claude Code wants
//! to …"), the agent's pointer on the card, and the panel under the Assistant button.

use super::native::{ActResult, Answer, Status, ASK_TIMEOUT};
use super::{tools, Permission};
use crate::app::{CardBuilderApp, Mode};
use crate::style::{icon, icons, semibold, P};
use crate::widgets::{button, BtnKind};
use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use lucide_icons::Icon;

const AMBER: Color32 = Color32::from_rgb(0xe0, 0xa8, 0x4a);
const GREEN: Color32 = Color32::from_rgb(0x4c, 0xc3, 0x8a);
const RED: Color32 = Color32::from_rgb(0xf0, 0x5a, 0x6a);

/// a heading that folds its part of the page away (SolidCraft's style): chevron, CAPITALS, an
/// optional count, and a rule
fn group(ui: &mut Ui, key: &str, title: &str, count: Option<usize>, add: impl FnOnce(&mut Ui)) {
    let id = ui.make_persistent_id(("prefs-group", key));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    ui.add_space(8.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, title));
    let p = ui.painter();
    let mut x = rect.left();
    p.text(pos2(x + 6.0, rect.center().y), Align2::CENTER_CENTER, icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }), FontId::new(12.0, icons()), P.muted);
    x += 18.0;
    let g = p.layout_no_wrap(title.to_uppercase(), FontId::new(11.0, semibold()), if resp.hovered() { P.text } else { P.muted });
    let tw = g.size().x;
    p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g, P.muted);
    x += tw + 8.0;
    if let Some(n) = count {
        let r = Rect::from_min_size(pos2(x, rect.center().y - 9.0), vec2(22.0, 18.0));
        p.rect_filled(r, CornerRadius::same(9), P.panel2);
        p.text(r.center(), Align2::CENTER_CENTER, n.to_string(), FontId::proportional(10.5), P.muted);
        x = r.right() + 8.0;
    }
    p.hline(x..=rect.right(), rect.center().y, Stroke::new(1.0, P.border));
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if open {
        ui.add_space(6.0);
        add(ui);
        ui.add_space(6.0);
    }
}

fn info(ui: &mut Ui, tip: &str) {
    ui.label(RichText::new(icon(Icon::Info)).family(icons()).size(13.0).color(P.muted)).on_hover_text(tip);
}

fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(11.5).color(P.muted)).wrap());
}

/// a status pill: coloured dot and words
fn pill(ui: &mut Ui, text: &str, col: Color32) {
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(11.5), col);
    let (r, _) = ui.allocate_exact_size(g.size() + vec2(26.0, 8.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same(10), col.gamma_multiply(0.16));
    ui.painter().circle_filled(pos2(r.left() + 10.0, r.center().y), 3.5, col);
    ui.painter().galley(pos2(r.left() + 18.0, r.center().y - g.size().y / 2.0), g, col);
}

fn status_line(s: &Status) -> (String, Color32) {
    match s {
        Status::Listening(p) => (format!("Listening on 127.0.0.1:{p}"), GREEN),
        Status::Off => ("Off".into(), P.muted),
        Status::Failed(p, e) => (format!("Port {p}: {e}"), RED),
    }
}

/// how to add CardBuilder to some agents: (who, command or config)
fn connect_lines(port: u16) -> Vec<(&'static str, String)> {
    let url = format!("http://127.0.0.1:{port}/mcp");
    vec![
        ("Claude Code", format!("claude mcp add --transport http --scope user cardbuilder {url}")),
        ("Gemini CLI", format!("gemini mcp add --transport http cardbuilder {url}")),
        ("Other MCP clients (JSON)", format!("{{\"mcpServers\": {{\"cardbuilder\": {{\"type\": \"http\", \"url\": \"{url}\"}}}}}}")),
    ]
}

fn copy_row(ui: &mut Ui, who: &str, text: &str) {
    ui.label(RichText::new(who).size(12.0).color(P.text));
    // the button first (at the right), then the line fills what is left
    ui.allocate_ui_with_layout(vec2(ui.available_width(), 30.0), Layout::right_to_left(Align::Center), |ui| {
        if button(ui, Some(Icon::Copy), "Copy", BtnKind::Normal, true).on_hover_text("Copy to the clipboard").clicked() {
            ui.ctx().copy_text(text.to_string());
        }
        let w = (ui.available_width() - 16.0).max(120.0);
        egui::Frame::NONE.fill(P.panel2).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.set_width(w);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(text).monospace().size(11.5).color(P.text)).truncate());
            });
        });
    });
    ui.add_space(4.0);
}

fn ago(secs: f64) -> String {
    match secs as u64 {
        0..=9 => "just now".into(),
        s @ 10..=59 => format!("{s} s ago"),
        s @ 60..=3599 => format!("{} min ago", s / 60),
        s => format!("{} h ago", s / 3600),
    }
}

/// the classic arrow pointer with its tip at `tip`
fn arrow(p: &egui::Painter, tip: Pos2, fill: Color32, alpha: f32) {
    let pts = [(0.0, 0.0), (0.0, 17.0), (4.6, 13.2), (7.6, 20.0), (10.4, 18.8), (7.4, 12.2), (12.6, 12.2)];
    let pts: Vec<Pos2> = pts.iter().map(|(x, y)| tip + vec2(*x, *y)).collect();
    p.add(egui::Shape::convex_polygon(pts.clone(), fill.gamma_multiply(alpha), Stroke::new(1.5, Color32::WHITE.gamma_multiply(alpha))));
}

impl CardBuilderApp {
    /// Preferences › Assistant
    pub(crate) fn assistant_page(&mut self, ui: &mut Ui, now: f64) {
        let mut a = self.prefs.assistant.clone();
        ui.label(RichText::new("Assistant").family(semibold()).size(16.0));
        note(ui, "AI agents that design with you in CardBuilder.");

        group(ui, "availability", "Availability", None, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut a.show_ai, "Show AI features");
                info(ui, "Off hides everything to do with AI: the Assistant button, the agent's pointer, and the connection for agents.");
            });
            ui.add_enabled(a.show_ai, egui::Checkbox::new(&mut a.app_bar_button, "Assistant button in the application bar"));
        });

        group(ui, "permissions", "Permissions", None, |ui| {
            let opts = [
                (Permission::AskFirst, "Ask first", "Asks for every change"),
                (Permission::MakeEdits, "Make edits", "Edits freely, asks for the rest"),
                (Permission::DoEverything, "Do everything", "Never asks"),
            ];
            let gap = 8.0;
            let w = ((ui.available_width() - 2.0 * gap) / 3.0).floor();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (perm, title, sub) in opts {
                    let on = a.permission == perm;
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, title));
                    let p = ui.painter();
                    let fill = if on { P.accent.gamma_multiply(0.18) } else if resp.hovered() { P.panel2 } else { P.panel };
                    p.rect(r, CornerRadius::same(8), fill, Stroke::new(if on { 1.5 } else { 1.0 }, if on { P.accent } else { P.border }), StrokeKind::Inside);
                    let c = pos2(r.left() + 16.0, r.top() + 19.0);
                    p.circle_stroke(c, 6.0, Stroke::new(1.5, if on { P.accent } else { P.muted }));
                    if on {
                        p.circle_filled(c, 3.2, P.accent);
                    }
                    let mut tx = r.left() + 28.0;
                    if perm == Permission::DoEverything {
                        p.text(pos2(tx, c.y), Align2::LEFT_CENTER, icon(Icon::TriangleAlert), FontId::new(13.0, icons()), AMBER);
                        tx += 18.0;
                    }
                    p.text(pos2(tx, c.y), Align2::LEFT_CENTER, title, FontId::new(13.0, semibold()), P.text);
                    p.text(pos2(r.left() + 14.0, r.bottom() - 15.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), P.muted);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        a.permission = perm;
                    }
                }
            });
            ui.add_space(4.0);
            if a.permission == Permission::DoEverything {
                ui.add(egui::Label::new(RichText::new("The agent can save, overwrite, export and replace your design without asking. Use this only with an agent you trust, on designs you have saved.").size(12.0).color(RED)).wrap());
            }
            note(ui, "Reading the design and looking at the card never ask. Everything an agent changes is one Ctrl+Z away; opening, saving, exporting and starting a new design are \"the rest\".");
        });

        group(ui, "always", "Always allowed", Some(a.always_allowed.len()), |ui| {
            if a.always_allowed.is_empty() {
                note(ui, "Nothing yet. When CardBuilder asks, Always allow puts that kind of change here.");
            }
            let mut drop = None;
            for (i, t) in a.always_allowed.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon(Icon::Check)).family(icons()).color(GREEN));
                    ui.label(tools::title(t));
                    ui.label(RichText::new(t).monospace().size(11.0).color(P.muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if crate::widgets::icon_button(ui, Icon::X, "Ask again", true).clicked() {
                            drop = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = drop {
                a.always_allowed.remove(i);
            }
        });

        let sessions = self.assistant.sessions();
        group(ui, "agents", "Agents", Some(sessions.len()), |ui| {
            if sessions.is_empty() {
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
                ui.painter().rect_stroke(r, CornerRadius::same(10), Stroke::new(1.0, P.border), StrokeKind::Inside);
                let words = if a.mcp && a.show_ai { "No agents connected. Add CardBuilder to your agent with a line below." } else { "No agents yet. Turn on Let AI agents connect (below), then add CardBuilder to your agent." };
                ui.painter().text(r.center(), Align2::CENTER_CENTER, words, FontId::proportional(12.5), P.muted);
            }
            for s in &sessions {
                egui::Frame::NONE.fill(P.panel2).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(10, 7)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon(Icon::Bot)).family(icons()).size(16.0).color(P.accent));
                        ui.label(RichText::new(&s.agent).family(semibold()));
                        if !s.version.is_empty() {
                            ui.label(RichText::new(&s.version).size(11.0).color(P.muted));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format!("connected {} · {} call{} · active {}", ago(s.since.elapsed().as_secs_f64()), s.calls, if s.calls == 1 { "" } else { "s" }, ago(s.last.elapsed().as_secs_f64()))).size(11.0).color(P.muted));
                        });
                    });
                });
                ui.add_space(4.0);
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Connect an agent").family(semibold()).size(12.5));
            note(ui, "Run one of these once on this computer; the agent then finds CardBuilder whenever the app is open.");
            ui.add_space(2.0);
            for (who, line) in connect_lines(a.port) {
                copy_row(ui, who, &line);
            }
        });

        group(ui, "instructions", "Custom instructions", None, |ui| {
            ui.add(egui::TextEdit::multiline(&mut a.instructions).desired_rows(5).desired_width(f32::INFINITY).hint_text("For example: Use our brand colours #0b3d91 and #f2a900. Keep text at 7 pt or bigger."));
            note(ui, "Sent to agents when they connect, after CardBuilder's own instructions: how you like to work, rules to keep to. A change reaches an agent when it next connects.");
        });

        group(ui, "pointer", "Agent pointer", None, |ui| {
            ui.horizontal_top(|ui| {
                let left_w = (ui.available_width() - 250.0).max(200.0);
                ui.vertical(|ui| {
                    ui.set_width(left_w);
                    ui.checkbox(&mut a.pointer, "Show the agent's pointer");
                    ui.add_enabled_ui(a.pointer, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Speed");
                            ui.spacing_mut().slider_width = (left_w - 130.0).max(80.0);
                            ui.add(egui::Slider::new(&mut a.pointer_speed, 0.25..=4.0).logarithmic(true).show_value(false));
                            ui.label(RichText::new(format!("{:.2}x", a.pointer_speed)).monospace().size(11.5));
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut a.follow, "Select what the agent changes");
                        info(ui, "The layer an agent changes becomes the selected one, so its settings show in the sidebar while you watch.");
                    });
                });
                // a little demonstration of the pointer at the chosen speed
                let (r, _) = ui.allocate_exact_size(vec2(230.0, 92.0), Sense::hover());
                let p = ui.painter();
                p.rect(r, CornerRadius::same(10), P.panel2, Stroke::new(1.0, P.border), StrokeKind::Inside);
                p.text(r.left_top() + vec2(10.0, 8.0), Align2::LEFT_TOP, if a.pointer { "Pointer" } else { "Pointer hidden" }, FontId::proportional(11.0), P.muted);
                let pts = [r.left_top() + vec2(30.0, 62.0), r.left_top() + vec2(120.0, 62.0), r.left_top() + vec2(200.0, 40.0)];
                for w in pts.windows(2) {
                    p.extend(egui::Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.0, P.muted), 4.0, 3.0));
                }
                for q in pts {
                    p.circle_filled(q, 3.5, P.muted);
                }
                if a.pointer {
                    let cycle = 2.4 / a.pointer_speed as f64;
                    let t = ((now % cycle) / cycle) as f32 * 2.0;
                    let (i, f) = ((t.floor() as usize).min(1), t.fract());
                    let e = f * f * (3.0 - 2.0 * f);
                    arrow(p, pts[i] + (pts[i + 1] - pts[i]) * e, P.accent, 1.0);
                    ui.ctx().request_repaint();
                }
            });
        });

        group(ui, "mcp", "Agents through MCP", None, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled(a.show_ai, egui::Checkbox::new(&mut a.mcp, "Let AI agents connect"));
                info(ui, "CardBuilder listens on this computer for agents that speak MCP (the Model Context Protocol), such as Claude Code or Gemini CLI. Only programs on this computer can connect, and web pages can't.");
                let (text, col) = status_line(&self.assistant.status);
                pill(ui, &text, col);
            });
            ui.horizontal(|ui| {
                ui.label("Port");
                let mut port = a.port as u32;
                if ui.add(egui::DragValue::new(&mut port).range(1024..=65535).speed(1.0)).on_hover_text(format!("{} unless another program uses it", super::DEFAULT_PORT)).changed() {
                    a.port = port as u16;
                }
                if a.port != super::DEFAULT_PORT && ui.small_button("Default").clicked() {
                    a.port = super::DEFAULT_PORT;
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut a.compact, "Compact output");
                info(ui, "Shorter tool results (no indentation, fewer details), which saves room in the agent's context.");
            });
        });
        ui.add_space(8.0);
        note(ui, "Only the agent you connect sees your design, and it talks to its own AI service. CardBuilder itself sends nothing anywhere.");
        if a != self.prefs.assistant {
            self.prefs.assistant = a;
        }
    }

    /// the Assistant button in the top bar (a dot when an agent is connected or waiting)
    pub(crate) fn assistant_button(&mut self, ui: &mut Ui) {
        let waiting = self.assistant.pending.is_some();
        let tip = if waiting { "An agent is waiting for your answer" } else { "Assistant: AI agents" };
        let r = crate::widgets::icon_button(ui, Icon::Sparkles, tip, true);
        let dot = if waiting { Some(AMBER) } else if !self.assistant.sessions().is_empty() { Some(GREEN) } else { None };
        if let Some(c) = dot {
            ui.painter().circle(r.rect.right_top() + vec2(-6.0, 6.0), 3.5, c, Stroke::new(2.0, P.panel));
        }
        if r.clicked() {
            self.assistant.panel = !self.assistant.panel;
        }
        self.assistant_anchor = r.rect;
    }

    /// the approval card, the agent's pointer and the Assistant panel
    pub(crate) fn assistant_overlays(&mut self, ctx: &egui::Context, now: f64) {
        let a = self.prefs.assistant.clone();
        if !a.show_ai {
            self.assistant.panel = false;
            return;
        }
        self.approval_card(ctx, now);
        if a.pointer && self.prefs.mode != Mode::ThreeD {
            self.draw_pointer(ctx, now, a.pointer_speed as f64);
        }
        // Esc, or a dialog opening, closes the panel
        if self.popups.prefs || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.assistant.panel = false;
        }
        if self.assistant.panel && a.app_bar_button {
            self.assistant_panel(ctx, now);
        }
    }

    fn approval_card(&mut self, ctx: &egui::Context, now: f64) {
        let Some(p) = &self.assistant.pending else { return };
        let (agent, what, tool, left) = (p.call.agent.clone(), p.what.clone(), p.call.tool.clone(), (ASK_TIMEOUT - (now - p.since)).max(0.0));
        let screen = ctx.content_rect();
        let mut answer = None;
        egui::Area::new(egui::Id::unique("assistant-approval"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::CENTER_BOTTOM, vec2(-160.0, -92.0))
            .interactable(true)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(P.menu)
                    .stroke(Stroke::new(1.5, P.accent))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(14))
                    .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(110) })
                    .show(ui, |ui| {
                        ui.set_width((screen.width() * 0.4).clamp(320.0, 520.0));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon(Icon::Bot)).family(icons()).size(18.0).color(P.accent));
                            ui.label(RichText::new(format!("{agent} wants to")).color(P.muted));
                        });
                        ui.add(egui::Label::new(RichText::new(&what).family(semibold()).size(15.0).color(P.text)).wrap());
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if button(ui, Some(Icon::Check), "Allow", BtnKind::Primary, true).clicked() {
                                answer = Some(Answer::Allow);
                            }
                            if button(ui, None, &format!("Always allow: {}", tools::title(&tool)), BtnKind::Normal, true).on_hover_text("Don't ask again for this kind of change (Preferences › Assistant › Always allowed)").clicked() {
                                answer = Some(Answer::Always);
                            }
                            if button(ui, Some(Icon::X), "Deny", BtnKind::Normal, true).clicked() {
                                answer = Some(Answer::Deny);
                            }
                        });
                        ui.label(RichText::new(format!("No answer in {left:.0} s counts as Deny")).size(11.0).color(P.muted));
                    });
            });
        if let Some(ans) = answer {
            self.answer(ans, now);
        }
    }

    fn draw_pointer(&mut self, ctx: &egui::Context, now: f64, speed: f64) {
        let Some((side, x, y)) = self.assistant.pointer.target else { return };
        let since = now - self.assistant.pointer.t0;
        let (move_s, stay) = (0.7 / speed, 4.0);
        if since > move_s + stay + 1.0 {
            self.assistant.pointer.target = None;
            self.assistant.pointer.at = None;
            return;
        }
        let to = self.canvas.to_screen(side, x, y);
        let from = self.assistant.pointer.from.unwrap_or(to + vec2(-60.0, -50.0));
        let t = (since / move_s).clamp(0.0, 1.0) as f32;
        let e = t * t * (3.0 - 2.0 * t);
        let at = from + (to - from) * e;
        let alpha = (1.0 - ((since - move_s - stay) as f32).clamp(0.0, 1.0)).clamp(0.0, 1.0);
        // only over the card editor (not the sidebar or the top bar)
        let screen = ctx.content_rect();
        let editor = Rect::from_min_max(pos2(screen.left(), screen.top() + 52.0), pos2(screen.right() - 320.0, screen.bottom()));
        if editor.contains(at) {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("assistant-pointer"))).with_clip_rect(editor);
            arrow(&painter, at, P.accent, alpha);
            let g = painter.layout_no_wrap(self.assistant.pointer.label.clone(), FontId::new(11.5, semibold()), Color32::WHITE);
            let r = Rect::from_min_size(at + vec2(14.0, 18.0), g.size() + vec2(14.0, 6.0));
            painter.rect_filled(r, CornerRadius::same(8), P.accent.gamma_multiply(alpha));
            painter.galley(r.min + vec2(7.0, 3.0), g, Color32::WHITE.gamma_multiply(alpha));
        }
        self.assistant.pointer.at = Some(at);
        ctx.request_repaint();
    }

    fn assistant_panel(&mut self, ctx: &egui::Context, now: f64) {
        let anchor = self.assistant_anchor;
        let port = self.prefs.assistant.port;
        let mut open_prefs = false;
        let mut connect = false;
        let resp = egui::Area::new(egui::Id::unique("assistant-panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2((anchor.right() - 360.0).max(8.0), anchor.bottom() + 8.0))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(P.menu)
                    .stroke(Stroke::new(1.0, P.border))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(14))
                    .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(100) })
                    .show(ui, |ui| {
                        ui.set_width(360.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Assistant").family(semibold()).size(15.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (text, col) = status_line(&self.assistant.status);
                                pill(ui, if matches!(self.assistant.status, Status::Listening(_)) { "Agents can connect" } else { &text }, col);
                            });
                        });
                        ui.add_space(4.0);
                        if !self.prefs.assistant.mcp {
                            note(ui, "Let AI agents on this computer, such as Claude Code, work on your design with you. You decide what they may do without asking.");
                            ui.add_space(4.0);
                            if button(ui, Some(Icon::Plug), "Let agents connect", BtnKind::Primary, true).clicked() {
                                connect = true;
                            }
                        } else {
                            let sessions = self.assistant.sessions();
                            if sessions.is_empty() {
                                note(ui, "No agent connected yet. Add CardBuilder to Claude Code once:");
                                let (who, line) = connect_lines(port).remove(0);
                                copy_row(ui, who, &line);
                            }
                            for s in &sessions {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(icon(Icon::Bot)).family(icons()).color(P.accent));
                                    ui.label(RichText::new(&s.agent).family(semibold()));
                                    ui.label(RichText::new(format!("· active {}", ago(s.last.elapsed().as_secs_f64()))).size(11.0).color(P.muted));
                                });
                            }
                        }
                        if !self.assistant.log.is_empty() {
                            ui.add_space(6.0);
                            ui.label(RichText::new("RECENT").size(10.5).color(P.muted));
                            for act in self.assistant.log.iter().take(7) {
                                // the time first (at the right), then the line, cut short to fit
                                ui.allocate_ui_with_layout(vec2(ui.available_width(), 20.0), Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(ago(now - act.time)).size(10.5).color(P.muted));
                                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                        let (ic, col) = match act.result {
                                            ActResult::Done => (Icon::Check, GREEN),
                                            ActResult::Denied => (Icon::Ban, AMBER),
                                            ActResult::Failed => (Icon::TriangleAlert, RED),
                                        };
                                        ui.label(RichText::new(icon(ic)).family(icons()).size(12.0).color(col));
                                        let line = format!("{}: {}", act.agent, act.what);
                                        ui.add(egui::Label::new(RichText::new(&line).size(12.0)).truncate()).on_hover_text(line);
                                    });
                                });
                            }
                        }
                        ui.add_space(8.0);
                        ui.separator();
                        if button(ui, Some(Icon::Settings), "Assistant settings…", BtnKind::Normal, true).clicked() {
                            open_prefs = true;
                        }
                    });
            })
            .response;
        if connect {
            self.prefs.assistant.mcp = true;
        }
        if open_prefs {
            self.assistant.panel = false;
            self.popups.prefs = true;
            self.popups.prefs_page = crate::preferences::PrefsPage::Assistant;
        }
        // a click anywhere else closes it
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !resp.rect.contains(p) && !anchor.contains(p)));
        if clicked_outside {
            self.assistant.panel = false;
        }
    }
}
