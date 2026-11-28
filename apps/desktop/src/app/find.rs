use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use eframe::egui::{self, RichText, ScrollArea, Ui};
use hexaris_engine::{
    Instruction, LinearListing, NumberBase, ReferenceKind, decompile, lift, scan_strings, string_at,
};

use super::{App, Session, View};

const MAX_HITS: usize = 200;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Scope {
    #[default]
    View,
    Binary,
}

#[derive(Clone, Copy)]
enum Destination {
    Code(u64),
    Pseudo(usize, Option<u64>),
    Hex(usize),
}

struct Hit {
    text: String,
    destination: Option<Destination>,
}

#[derive(Default)]
pub(super) struct FindState {
    open: bool,
    focus: bool,
    scope: Scope,
    query: String,
    submitted: String,
    origin: Option<(View, Option<u64>, bool, NumberBase)>,
    hits: Vec<Hit>,
    worker: Option<Receiver<Vec<Hit>>>,
}

impl FindState {
    pub(super) fn open(&mut self) {
        self.open = true;
        self.focus = true;
    }

    pub(super) fn toggle(&mut self, ui: &mut Ui) {
        if ui.small_button("Find text  Ctrl+F").clicked() {
            if self.open {
                self.open = false;
            } else {
                self.open();
            }
        }
    }

    pub(super) fn matches(&self, text: &str) -> bool {
        self.open
            && self.scope == Scope::View
            && !self.submitted.is_empty()
            && text.to_lowercase().contains(&self.submitted)
    }
}

impl App {
    pub(super) fn find_text(&mut self, ui: &mut Ui) {
        let find = &mut self.state.find;
        let Some(session) = &self.session else { return };
        if let Some(worker) = &find.worker {
            match worker.try_recv() {
                Ok(hits) => {
                    if find.scope == Scope::Binary && !find.submitted.is_empty() {
                        find.hits = hits;
                    }
                    find.worker = None;
                }
                Err(TryRecvError::Disconnected) => find.worker = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if !find.open {
            return;
        }
        let view = self.state.view;
        let function = self.state.selected_function;
        let pseudo_control_flow =
            self.state.pseudo_control_flow || self.state.selection.range.is_some();
        let scoped_function = if view == View::Program {
            None
        } else {
            function
        };
        let origin = (
            view,
            scoped_function,
            pseudo_control_flow,
            self.state.number_base,
        );
        if find.scope == Scope::View && find.origin != Some(origin) {
            find.hits.clear();
            find.submitted.clear();
        }
        let mut submit = false;
        ui.horizontal_wrapped(|ui| {
            let previous = find.scope;
            ui.selectable_value(&mut find.scope, Scope::View, "Current view");
            ui.selectable_value(&mut find.scope, Scope::Binary, "Binary strings");
            if previous != find.scope {
                find.hits.clear();
                find.submitted.clear();
            }
            let field = ui.add(
                egui::TextEdit::singleline(&mut find.query)
                    .hint_text("Find text…")
                    .char_limit(256)
                    .desired_width(180.0),
            );
            if std::mem::take(&mut find.focus) {
                field.request_focus();
            }
            if field.changed() {
                find.hits.clear();
                find.submitted.clear();
            }
            let ready = find.worker.is_none() && !find.query.trim().is_empty();
            submit = ui.add_enabled(ready, egui::Button::new("Find")).clicked()
                || (ready
                    && field.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter)));
            if ui.small_button("Close").clicked() {
                find.open = false;
            }
        });
        if submit {
            find.hits.clear();
            find.submitted = find.query.trim().to_lowercase();
            if find.scope == Scope::Binary
                || matches!(view, View::Hex | View::Strings | View::Search)
            {
                find.scope = Scope::Binary;
                let bytes = session.project.image().shared_bytes();
                let pattern = find.submitted.clone();
                let ctx = ui.ctx().clone();
                let (tx, rx) = mpsc::channel();
                find.worker = Some(rx);
                thread::spawn(move || {
                    let hits = scan_strings(&bytes, 1)
                        .filter(|item| item.ascii.to_lowercase().contains(&pattern))
                        .take(MAX_HITS + 1)
                        .map(|item| Hit {
                            text: format!("0x{:X}   {:?}", item.offset, item.ascii),
                            destination: Some(Destination::Hex(item.offset)),
                        })
                        .collect();
                    let _ = tx.send(hits);
                    ctx.request_repaint();
                });
            } else {
                find.hits =
                    search_view(session, origin, self.state.linear.as_ref(), &find.submitted);
                find.origin = Some(origin);
            }
        }
        let mut jump = None;
        if find.worker.is_some() {
            super::label(ui, "Searching binary strings…");
        } else if !find.submitted.is_empty() {
            let total = find.hits.len();
            let caption = match total {
                0 => "No matches".into(),
                1 => "1 match".into(),
                n if n > MAX_HITS => format!("First {MAX_HITS} matches"),
                n => format!("{n} matches"),
            };
            super::label(ui, &caption);
            ScrollArea::vertical()
                .id_salt(("find-results", self.version))
                .max_height(126.0)
                .show(ui, |ui| {
                    for hit in find.hits.iter().take(MAX_HITS) {
                        let text = RichText::new(&hit.text).monospace();
                        if let Some(destination) = hit.destination {
                            let link = ui.link(text);
                            let link = if matches!(destination, Destination::Hex(_)) {
                                link.on_hover_text("Open these bytes in Hex")
                            } else {
                                link
                            };
                            if link.clicked() {
                                jump = Some(destination);
                            }
                        } else {
                            ui.label(text);
                        }
                    }
                });
        }
        match jump {
            Some(Destination::Code(address)) => {
                let owner = function
                    .filter(|key| {
                        session.analysis.functions.get(key).is_some_and(|item| {
                            item.instructions
                                .binary_search_by_key(&address, |instruction| instruction.address)
                                .is_ok()
                        })
                    })
                    .or_else(|| session.references.owner(address))
                    .or(function);
                if let Some(function) = owner {
                    self.navigate(function, address);
                    if view == View::Flow {
                        self.state.flow.focus_selection = true;
                    } else if view == View::Program {
                        self.state.program_jump = Some(address);
                    }
                }
            }
            Some(Destination::Pseudo(row, address)) => {
                if let Some((function, address)) = function.zip(address) {
                    self.navigate(function, address);
                }
                self.state.pseudo_jump = Some(row);
            }
            Some(Destination::Hex(offset)) => self.show_hex(offset),
            None => {}
        }
    }
}

fn search_view(
    session: &Session,
    (view, function, pseudo_control_flow, base): (View, Option<u64>, bool, NumberBase),
    linear: Option<&LinearListing>,
    pattern: &str,
) -> Vec<Hit> {
    if view == View::Program {
        return session
            .program
            .instructions
            .iter()
            .filter_map(|row| {
                let (function, instruction) = row.resolve(&session.analysis);
                let name = session.function_name(function);
                let comment = session.project.comment(row.address).unwrap_or_default();
                let text = format!(
                    "0x{:X}   {}   {name}   {comment}",
                    row.address,
                    format_instruction(instruction, base)
                );
                text.to_lowercase().contains(pattern).then_some(Hit {
                    text,
                    destination: Some(Destination::Code(row.address)),
                })
            })
            .take(MAX_HITS + 1)
            .collect();
    }
    let mut hits = Vec::new();
    let mut add = |text: &str, destination| {
        if hits.len() <= MAX_HITS && text.to_lowercase().contains(pattern) {
            hits.push(Hit {
                text: text.to_owned(),
                destination,
            });
        }
    };
    let image = session.project.image();
    if view == View::Overview {
        let info = &image.info;
        for text in [
            session.project.name(),
            info.format,
            info.architecture.name(),
            &session.triage.sha256,
        ] {
            add(text, None);
        }
        for import in &info.imports {
            add(&format!("{}!{}", import.library, import.name), None);
        }
        for region in info.sections.iter().chain(&info.segments) {
            add(
                &region.name,
                region
                    .file_offset
                    .and_then(|offset| usize::try_from(offset).ok())
                    .map(Destination::Hex),
            );
        }
        return hits;
    }
    if view == View::Linear
        && let Some(linear) = linear
    {
        for instruction in &linear.instructions {
            add(
                &format!(
                    "0x{:X}   {}",
                    instruction.address,
                    format_instruction(instruction, base)
                ),
                Some(Destination::Code(instruction.address)),
            );
        }
    }
    let Some(function) = function.and_then(|address| session.analysis.functions.get(&address))
    else {
        return hits;
    };
    let mut search_references = true;
    match view {
        View::Pseudocode => {
            let rendered;
            let result = if let Some(cached) = session.pseudocode.get(&function.address) {
                cached
            } else {
                rendered = decompile(image, function);
                &rendered
            };
            if let Some(code) = result.code.as_ref().filter(|_| !pseudo_control_flow) {
                let code =
                    super::rename::named_structured(&session.project, function.address, code);
                for (index, line) in code.lines().enumerate() {
                    add(line, Some(Destination::Pseudo(index, None)));
                }
            } else if let Some(listing) = &result.control_flow {
                search_references = false;
                for (index, line) in listing.lines.iter().enumerate() {
                    add(&line.text, Some(Destination::Pseudo(index, line.address)));
                }
            }
        }
        View::Registers => {
            let translated = lift(function);
            for block in translated.blocks {
                for statement in block.statements {
                    add(&statement.text, Some(Destination::Code(statement.address)));
                }
            }
        }
        View::Calls => {
            for site in session
                .references
                .callers(function.address)
                .iter()
                .chain(session.references.callees(function.address))
            {
                let name = session
                    .analysis
                    .functions
                    .get(&site.target)
                    .map(|f| session.function_name(f))
                    .unwrap_or_default();
                add(
                    &format!("{name} · 0x{:X} → 0x{:X}", site.site, site.target),
                    Some(Destination::Code(site.site)),
                );
            }
        }
        View::Disassembly | View::Flow => {
            for instruction in &function.instructions {
                let comment = session
                    .project
                    .comment(instruction.address)
                    .unwrap_or_default();
                add(
                    &format!(
                        "0x{:X}   {}   {comment}",
                        instruction.address,
                        format_instruction(instruction, base)
                    ),
                    Some(Destination::Code(instruction.address)),
                );
            }
        }
        _ => {}
    }
    if search_references {
        for reference in &function.references {
            if reference.kind == ReferenceKind::Address
                && let Some(text) = string_at(image, reference.target)
            {
                add(
                    &format!("0x{:X}   {text:?}", reference.source),
                    Some(Destination::Code(reference.source)),
                );
            }
        }
    }
    hits
}

fn format_instruction(instruction: &Instruction, base: NumberBase) -> String {
    if base == NumberBase::Hex {
        return instruction.text.clone();
    }
    let mut text = String::new();
    instruction.visit_tokens_in(base, |token, _| text.push_str(token));
    text
}
