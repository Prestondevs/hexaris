use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use eframe::egui::{self, Button, Context, Key, RichText, ScrollArea, TextureHandle, Ui};
use hexaris_engine::{
    Analysis, Architecture, BytePatch, BytePattern, Decompilation, Function, Image, ImageTriage,
    LiftedFunction, LinearListing, LoadMode, Project, ReferenceIndex, StringItem, analyze,
    analyze_function, decompile, disassemble, import_ghidra_xml, lift, parse_address, triage,
};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use crate::theme::{self, Theme};

mod editing;
mod find;
mod flow;
mod rename;
mod selection;
mod tools;
mod values;
mod views;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum View {
    Overview,
    #[default]
    Disassembly,
    Linear,
    Flow,
    Calls,
    Registers,
    Pseudocode,
    Hex,
    Strings,
    Search,
}

impl View {
    const ALL: [Self; 10] = [
        Self::Overview,
        Self::Disassembly,
        Self::Linear,
        Self::Flow,
        Self::Calls,
        Self::Registers,
        Self::Pseudocode,
        Self::Hex,
        Self::Strings,
        Self::Search,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Disassembly => "Disassembly",
            Self::Linear => "Linear",
            Self::Flow => "Control flow",
            Self::Calls => "Calls",
            Self::Registers => "Register IR",
            Self::Pseudocode => "Pseudo-C",
            Self::Hex => "Hex",
            Self::Strings => "Strings",
            Self::Search => "Byte search",
        }
    }

    fn is_code(self) -> bool {
        matches!(
            self,
            Self::Disassembly
                | Self::Linear
                | Self::Flow
                | Self::Calls
                | Self::Registers
                | Self::Pseudocode
        )
    }
}

struct Session {
    source: PathBuf,
    load_mode: LoadMode,
    project: Project,
    analysis: Analysis,
    references: ReferenceIndex,
    triage: ImageTriage,
    path: Option<PathBuf>,
    dirty: bool,
    lifted: BTreeMap<u64, LiftedFunction>,
    pseudocode: BTreeMap<u64, Decompilation>,
    patches: Vec<BytePatch>,
}

impl Session {
    fn has_unsaved_work(&self) -> bool {
        self.dirty || !self.patches.is_empty()
    }

    fn function_name<'a>(&'a self, function: &'a Function) -> &'a str {
        self.project
            .function_name(function.address)
            .unwrap_or(&function.name)
    }
}

enum QueryResult {
    Bytes(Vec<usize>),
    Strings(Vec<StringItem>),
}

#[derive(Default)]
struct ViewState {
    view: View,
    preferred_code_view: View,
    reset_code_view: bool,
    filter: String,
    browser_tab: usize,
    selected_function: Option<u64>,
    selected_address: Option<u64>,
    selected_file_offset: Option<usize>,
    hex_jump: Option<usize>,
    disassembly_jump: Option<usize>,
    linear: Option<LinearListing>,
    jump_text: String,
    hex_offset: String,
    note: String,
    name: String,
    bookmark: String,
    query: String,
    strings_min: usize,
    strings: Option<Vec<StringItem>>,
    hits: Option<Vec<usize>>,
    querying: Option<Receiver<QueryResult>>,
    navigation_back: Vec<(u64, u64)>,
    navigation_forward: Vec<(u64, u64)>,
    flow: flow::GraphState,
    pseudo_control_flow: bool,
    find: find::FindState,
    pseudo_jump: Option<usize>,
    number_base: hexaris_engine::NumberBase,
    selection: selection::AddressSelection,
}

struct BackgroundTab {
    session: Session,
    state: ViewState,
}

pub struct App {
    theme: Theme,
    session: Option<Session>,
    background: BTreeMap<u64, BackgroundTab>,
    state: ViewState,
    opening: Option<Receiver<hexaris_engine::Result<Session>>>,
    queued: VecDeque<(PathBuf, LoadMode)>,
    version: u64,
    next_version: u64,
    message: Option<(bool, String)>,
    show_inspector: bool,
    show_raw: bool,
    patch_dialog: Option<editing::PatchDialog>,
    tools_dialog: Option<tools::ToolsDialog>,
    raw_base: String,
    raw_arch: Architecture,
    mark: Option<TextureHandle>,
    values: values::Converter,
    rename: Option<rename::RenameDialog>,
    open_icon: Option<TextureHandle>,
    save_icon: Option<TextureHandle>,
    search_icon: Option<TextureHandle>,
    inspector_icon: Option<TextureHandle>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, paths: Vec<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        let theme = Theme::restore(cc.storage);
        theme::install(ctx, theme);
        let mut app = Self {
            theme,
            session: None,
            background: BTreeMap::new(),
            state: ViewState::default(),
            opening: None,
            queued: VecDeque::new(),
            version: 0,
            next_version: 1,
            message: None,
            show_inspector: true,
            show_raw: false,
            patch_dialog: None,
            tools_dialog: None,
            raw_base: "0x1000".into(),
            raw_arch: Architecture::X86_64,
            mark: theme::brand_texture(ctx),
            values: values::Converter::default(),
            rename: None,
            open_icon: theme::tabler(
                ctx,
                "open-file",
                include_str!("../../../assets/icons/tabler/folder-open.svg"),
            ),
            save_icon: theme::tabler(
                ctx,
                "save-project",
                include_str!("../../../assets/icons/tabler/device-floppy.svg"),
            ),
            search_icon: theme::tabler(
                ctx,
                "byte-search",
                include_str!("../../../assets/icons/tabler/search.svg"),
            ),
            inspector_icon: theme::tabler(
                ctx,
                "inspector",
                include_str!("../../../assets/icons/tabler/adjustments.svg"),
            ),
        };
        app.enqueue_files(ctx, paths, LoadMode::Auto);
        app
    }

    fn error(&mut self, message: impl ToString) {
        self.message = Some((true, message.to_string()));
    }

    fn confirm_discard(description: &str) -> bool {
        MessageDialog::new()
            .set_title("Unsaved analysis changes")
            .set_description(description)
            .set_buttons(MessageButtons::YesNo)
            .set_level(MessageLevel::Warning)
            .show()
            == MessageDialogResult::Yes
    }

    fn choose(&mut self, ctx: &Context, mode: LoadMode) {
        if self.opening.is_some() {
            return;
        }
        let dialog = FileDialog::new().set_title("Open binaries or Hexaris projects");
        let paths = if mode == LoadMode::Auto {
            dialog.pick_files().unwrap_or_default()
        } else {
            dialog.pick_file().into_iter().collect()
        };
        self.enqueue_files(ctx, paths, mode);
    }

    fn enqueue_files(&mut self, ctx: &Context, paths: Vec<PathBuf>, mode: LoadMode) {
        if paths.is_empty() {
            return;
        }
        self.message = None;
        self.queued
            .extend(paths.into_iter().map(|path| (path, mode)));
        self.start_next_file(ctx);
    }

    fn start_next_file(&mut self, ctx: &Context) {
        if self.opening.is_some() {
            return;
        }
        let Some((path, mode)) = self.queued.pop_front() else {
            return;
        };
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.opening = Some(rx);
        thread::spawn(move || {
            let _ = tx.send(load(path, mode));
            ctx.request_repaint();
        });
    }

    fn poll_workers(&mut self, ctx: &Context) {
        if let Some(worker) = &self.opening {
            match worker.try_recv() {
                Ok(result) => {
                    self.opening = None;
                    match result {
                        Ok(session) => self.open_tab(ctx, session),
                        Err(error) => self.error(error),
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.opening = None;
                    self.error("The loader exited without a result.");
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        self.start_next_file(ctx);
        if let Some(worker) = &self.state.querying {
            match worker.try_recv() {
                Ok(QueryResult::Bytes(hits)) => {
                    self.state.hits = Some(hits);
                    self.state.querying = None;
                }
                Ok(QueryResult::Strings(items)) => {
                    self.state.strings = Some(items);
                    self.state.querying = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.state.querying = None;
                    self.error("The search exited without a result.");
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    fn update_title(&self, ctx: &Context) {
        let title = self.session.as_ref().map_or_else(
            || "Hexaris".to_owned(),
            |session| format!("{}  |  Hexaris", session.project.name()),
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    fn stash_active(&mut self) {
        if let Some(session) = self.session.take() {
            let state = std::mem::take(&mut self.state);
            self.background
                .insert(self.version, BackgroundTab { session, state });
        }
        self.patch_dialog = None;
        self.rename = None;
    }

    fn open_tab(&mut self, ctx: &Context, session: Session) {
        let exists = |tab: &Session| {
            (tab.source == session.source && tab.load_mode == session.load_mode)
                || (session.path.is_some() && tab.path == session.path)
        };
        if self.session.as_ref().is_some_and(exists) {
            return;
        }
        if let Some((&id, _)) = self.background.iter().find(|(_, tab)| exists(&tab.session)) {
            self.activate(ctx, id);
            return;
        }
        let entry = session
            .project
            .image()
            .info
            .entry
            .filter(|address| session.analysis.functions.contains_key(address));
        let address = entry.or_else(|| session.analysis.functions.keys().next().copied());
        self.stash_active();
        self.session = Some(session);
        self.version = self.next_version;
        self.next_version += 1;
        self.state = ViewState {
            view: if address.is_some() {
                View::Disassembly
            } else {
                View::Hex
            },
            hex_jump: Some(0),
            strings_min: 5,
            ..Default::default()
        };
        self.select(address, address);
        self.update_title(ctx);
    }

    fn activate(&mut self, ctx: &Context, id: u64) {
        let Some(tab) = self.background.remove(&id) else {
            return;
        };
        self.stash_active();
        self.session = Some(tab.session);
        self.state = tab.state;
        self.version = id;
        self.message = None;
        self.update_title(ctx);
    }

    fn close_tab(&mut self, ctx: &Context, id: u64) {
        let session = if self.version == id {
            self.session.as_ref()
        } else {
            self.background.get(&id).map(|tab| &tab.session)
        };
        let Some(session) = session else { return };
        if session.has_unsaved_work()
            && !Self::confirm_discard(&format!(
                "Discard unsaved changes to {}?",
                session.project.name()
            ))
        {
            return;
        }
        if self.version != id {
            self.background.remove(&id);
            return;
        }
        self.session = None;
        self.state = ViewState::default();
        self.patch_dialog = None;
        self.message = None;
        let next = self
            .background
            .range(id..)
            .next()
            .or_else(|| self.background.last_key_value())
            .map(|(&id, _)| id);
        if let Some(next) = next {
            self.activate(ctx, next);
        } else {
            self.update_title(ctx);
        }
    }

    fn confirm_exit(&self) -> bool {
        let dirty = self
            .session
            .iter()
            .chain(self.background.values().map(|tab| &tab.session))
            .filter(|session| session.has_unsaved_work())
            .count();
        dirty == 0
            || Self::confirm_discard(&format!(
                "Discard unsaved changes in {dirty} project{}?",
                if dirty == 1 { "" } else { "s" }
            ))
    }

    fn save(&mut self, save_as: bool) {
        if self.opening.is_some() {
            return;
        }
        let Some(session) = &mut self.session else {
            return;
        };
        let existing = session.path.clone().filter(|_| !save_as);
        let path = match existing {
            Some(path) => path,
            None => {
                let Some(mut path) = FileDialog::new()
                    .set_title("Save Hexaris project")
                    .add_filter("Hexaris project", &["hxproj"])
                    .set_file_name(format!("{}.hxproj", session.project.name()))
                    .save_file()
                else {
                    return;
                };
                if !is_project(&path) {
                    path.set_extension("hxproj");
                }
                path
            }
        };
        let result = if session.path.as_deref() == Some(path.as_path()) && !save_as {
            session.project.save(&path)
        } else {
            session.project.save_new(&path)
        };
        match result {
            Ok(()) => {
                session.path = Some(path.canonicalize().unwrap_or_else(|_| path.clone()));
                session.dirty = false;
                self.message = Some((false, format!("Saved {}", path.display())));
            }
            Err(error) => self.error(error),
        }
    }

    fn select(&mut self, function: Option<u64>, address: Option<u64>) {
        if self.state.selected_function != function {
            self.state.selection.clear();
            self.state.pseudo_jump = None;
        }
        self.state.selected_function = function;
        self.state.selected_address = address;
        let session = self.session.as_ref();
        self.state.selected_file_offset =
            address.and_then(|address| session?.project.image().file_offset(address));
        self.state.note = address
            .and_then(|address| session?.project.comment(address))
            .unwrap_or_default()
            .into();
        self.state.name = function
            .and_then(|address| session?.project.function_name(address))
            .unwrap_or_default()
            .into();
        self.state.bookmark = address
            .and_then(|address| session?.project.bookmark(address))
            .unwrap_or_default()
            .into();
        self.state.disassembly_jump = function.and_then(|function| {
            session?
                .analysis
                .functions
                .get(&function)?
                .instructions
                .binary_search_by_key(&address?, |i| i.address)
                .ok()
        });
    }

    fn navigate(&mut self, function: u64, address: u64) {
        if let Some(before) = self
            .state
            .selected_function
            .zip(self.state.selected_address)
            .filter(|&before| before != (function, address))
        {
            self.state.navigation_back.push(before);
            self.state.navigation_forward.clear();
        }
        self.select(Some(function), Some(address));
    }

    fn history(&mut self, forward: bool) {
        let (from, to) = if forward {
            (
                &mut self.state.navigation_forward,
                &mut self.state.navigation_back,
            )
        } else {
            (
                &mut self.state.navigation_back,
                &mut self.state.navigation_forward,
            )
        };
        if let Some((function, address)) = from.pop() {
            to.extend(
                self.state
                    .selected_function
                    .zip(self.state.selected_address),
            );
            self.select(Some(function), Some(address));
            self.follow_code_view(address);
        }
    }

    fn follow_code_view(&mut self, address: u64) {
        self.state.view = if self.state.reset_code_view {
            View::Disassembly
        } else {
            self.state.preferred_code_view
        };
        if self.state.view == View::Linear {
            self.decode_linear(address);
        }
    }

    fn bookmark(&mut self, address: u64, name: String) {
        let Some(session) = &mut self.session else {
            return;
        };
        match session.project.set_bookmark(address, name) {
            Ok(()) => {
                session.dirty = true;
                self.state.bookmark = session.project.bookmark(address).unwrap_or_default().into();
            }
            Err(error) => self.error(error),
        }
    }

    fn import_ghidra(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(path) = FileDialog::new()
            .set_title("Import Ghidra program XML for this image")
            .add_filter("Ghidra program XML", &["xml"])
            .pick_file()
        else {
            return;
        };
        match import_ghidra_xml(&mut session.project, &path) {
            Ok(report) => {
                session.dirty |= report.added() > 0;
                self.select(self.state.selected_function, self.state.selected_address);
                self.message = Some((
                    false,
                    format!(
                        "Imported {} names, {} comments, {} bookmarks · {} kept · {} skipped",
                        report.function_names,
                        report.comments,
                        report.bookmarks,
                        report.existing,
                        report.skipped
                    ),
                ));
            }
            Err(error) => self.error(error),
        }
    }

    fn goto(&mut self, address: u64) {
        let Some(session) = &self.session else { return };
        let owner = if session.analysis.functions.contains_key(&address) {
            Some(address)
        } else {
            session.references.owner(address)
        };
        if let Some(function) = owner {
            self.navigate(function, address);
            self.follow_code_view(address);
        } else if let Some(offset) = session.project.image().file_offset(address) {
            self.show_hex(offset);
        } else {
            self.error(format!("0x{address:X} is not backed by file bytes."));
        }
    }

    fn show_hex(&mut self, offset: usize) {
        self.select_file_offset(offset);
        self.state.hex_jump = Some(offset);
        self.state.view = View::Hex;
    }

    fn select_file_offset(&mut self, offset: usize) {
        let address = self
            .session
            .as_ref()
            .and_then(|s| s.project.image().virtual_address(offset));
        self.select(self.state.selected_function, address);
        self.state.selected_file_offset = Some(offset);
    }

    fn analyze_at(&mut self, address: u64) {
        let Some(session) = &mut self.session else {
            return;
        };
        match analyze_function(session.project.image(), address) {
            Ok(function) => {
                session.analysis.functions.insert(address, function);
                session.references = ReferenceIndex::build(&session.analysis);
                self.state.flow = flow::GraphState::default();
                session.lifted.remove(&address);
                session.pseudocode.remove(&address);
                self.navigate(address, address);
                self.follow_code_view(address);
            }
            Err(error) => self.error(error),
        }
    }

    fn decode_linear(&mut self, address: u64) {
        let Some(session) = &self.session else { return };
        match disassemble(session.project.image(), address, 512) {
            Ok(listing) => {
                self.state.linear = Some(listing);
                self.select(self.state.selected_function, Some(address));
                self.state.view = View::Linear;
            }
            Err(error) => self.error(error),
        }
    }

    fn query(&mut self, ctx: &Context, strings: bool) {
        let Some(session) = &self.session else { return };
        if self.state.querying.is_some() {
            return;
        }
        let pattern = if strings {
            None
        } else {
            match BytePattern::parse(&self.state.query) {
                Ok(pattern) => Some(pattern),
                Err(error) => {
                    self.error(error);
                    return;
                }
            }
        };
        let bytes = session.project.image().shared_bytes();
        let minimum = self.state.strings_min;
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.state.querying = Some(rx);
        thread::spawn(move || {
            let result = match pattern {
                Some(pattern) => QueryResult::Bytes(pattern.find(&bytes).take(1001).collect()),
                None => QueryResult::Strings(
                    hexaris_engine::scan_strings(&bytes, minimum)
                        .take(1001)
                        .collect(),
                ),
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn top(&mut self, ctx: &Context) {
        let palette = self.theme.palette();
        let mut next_theme = None;
        egui::TopBottomPanel::top("application_header")
            .frame(theme::frame(palette.panel, 13, 9))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(mark) = &self.mark {
                        ui.image((mark.id(), egui::vec2(29.0, 29.0)));
                    }
                    ui.label(RichText::new("Hexaris").size(21.0).strong());
                    ui.add_space(13.0);
                    if ui
                        .add_enabled(
                            self.opening.is_none(),
                            icon_button(&self.open_icon, "Open", palette.text),
                        )
                        .on_hover_text("Open binary or project (Ctrl+O)")
                        .clicked()
                    {
                        self.choose(ctx, LoadMode::Auto);
                    }
                    ui.menu_button("File", |ui| {
                        if ui.button("Open raw image…").clicked() {
                            self.show_raw = true;
                            ui.close();
                        }
                        if ui
                            .add_enabled(self.session.is_some(), Button::new("Import Ghidra XML…"))
                            .clicked()
                        {
                            self.import_ghidra();
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.session.is_some() && self.opening.is_none(),
                                Button::new("Save project as…"),
                            )
                            .clicked()
                        {
                            self.save(true);
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.session.is_some() && self.opening.is_none(),
                                Button::new("Patch bytes…"),
                            )
                            .clicked()
                        {
                            self.open_patch(self.state.selected_file_offset);
                            ui.close();
                        }
                    });
                    if ui
                        .add_enabled(
                            self.session.is_some() && self.opening.is_none(),
                            icon_button(&self.save_icon, "Save project", palette.text),
                        )
                        .on_hover_text("Save project (Ctrl+S)")
                        .clicked()
                    {
                        self.save(false);
                    }
                    if let Some(session) = &self.session {
                        let title = if session.patches.is_empty() {
                            "Patch bytes…".into()
                        } else {
                            format!("Patches ({})", session.patches.len())
                        };
                        if ui.button(title).clicked() {
                            self.open_patch(self.state.selected_file_offset);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("Themes", |ui| {
                            ui.set_min_width(220.0);
                            for candidate in Theme::ALL {
                                ui.horizontal(|ui| {
                                    let named = ui.add_sized(
                                        [118.0, 23.0],
                                        Button::selectable(
                                            self.theme == candidate,
                                            candidate.name(),
                                        ),
                                    );
                                    let swatch = candidate.previews(ui);
                                    if named.clicked() || swatch {
                                        next_theme = Some(candidate);
                                        ui.close();
                                    }
                                });
                            }
                        });
                        if ui
                            .add_enabled(
                                self.session.is_some(),
                                icon_button(&self.inspector_icon, "Inspector", palette.text)
                                    .selected(self.show_inspector && self.session.is_some()),
                            )
                            .clicked()
                        {
                            self.show_inspector = !self.show_inspector;
                        }
                        if ui
                            .add_enabled(
                                self.session.is_some(),
                                icon_button(&self.search_icon, "Find bytes", palette.text),
                            )
                            .clicked()
                        {
                            self.state.view = View::Search;
                        }
                        self.tools_menu(ui);
                    });
                });
            });
        if let Some(theme) = next_theme {
            self.theme = theme;
            theme::install(ctx, theme);
            ctx.request_repaint();
        }
    }

    fn status(&mut self, ctx: &Context) {
        let palette = self.theme.palette();
        egui::TopBottomPanel::bottom("status")
            .frame(theme::frame(palette.panel, 13, 5))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.opening.is_some() {
                        ui.spinner();
                        ui.label("Opening and analyzing…");
                    } else if let Some((error, message)) = &self.message {
                        ui.add(
                            egui::Label::new(RichText::new(message).color(if *error {
                                palette.warn
                            } else {
                                palette.accent
                            }))
                            .truncate(),
                        );
                        if ui.small_button("Dismiss").clicked() {
                            self.message = None;
                        }
                    } else if let Some(session) = &self.session {
                        let info = &session.project.image().info;
                        ui.label(
                            RichText::new(format!(
                                "{}  ·  {}  ·  {} bytes",
                                info.format,
                                info.architecture.name(),
                                session.project.image().bytes().len()
                            ))
                            .color(palette.muted),
                        );
                    } else {
                        ui.label(RichText::new("Ready to open a local file").color(palette.muted));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.state.view == View::Hex
                            && let Some(offset) = self.state.selected_file_offset
                        {
                            ui.label(
                                RichText::new(format!("File 0x{offset:X}"))
                                    .monospace()
                                    .color(palette.muted),
                            );
                        }
                        if let Some(address) = self.state.selected_address {
                            ui.label(
                                RichText::new(format!("0x{address:X}"))
                                    .monospace()
                                    .color(palette.muted),
                            );
                        }
                    });
                });
            });
    }

    fn document_tabs(&mut self, ctx: &Context) {
        let Some(active) = &self.session else { return };
        let palette = self.theme.palette();
        let mut tabs: Vec<_> = self
            .background
            .iter()
            .map(|(&id, tab)| (id, &tab.session))
            .chain([(self.version, active)])
            .collect();
        tabs.sort_unstable_by_key(|(id, _)| *id);
        let mut select = None;
        let mut close = None;
        egui::TopBottomPanel::top("document_tabs")
            .frame(theme::frame(palette.raised, 12, 4))
            .show(ctx, |ui| {
                ScrollArea::horizontal()
                    .id_salt("document_tabs_scroll")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (id, session) in tabs {
                                ui.push_id(id, |ui| {
                                    let name = session.project.name();
                                    let mut display: String = name.chars().take(28).collect();
                                    if display.len() < name.len() {
                                        display.push('…');
                                    }
                                    if session.has_unsaved_work() {
                                        display.push_str(" •");
                                    }
                                    let tab = Button::selectable(id == self.version, display);
                                    let tooltip = session
                                        .path
                                        .as_ref()
                                        .unwrap_or(&session.source)
                                        .display()
                                        .to_string();
                                    if ui.add(tab).on_hover_text(tooltip).clicked() {
                                        select = Some(id);
                                    }
                                    if ui
                                        .small_button("×")
                                        .on_hover_text(format!("Close {name}"))
                                        .clicked()
                                    {
                                        close = Some(id);
                                    }
                                });
                                ui.separator();
                            }
                        });
                    });
            });
        if let Some(id) = close {
            self.close_tab(ctx, id);
        } else if let Some(id) = select {
            self.activate(ctx, id);
        }
    }

    fn raw_window(&mut self, ctx: &Context) {
        let mut visible = self.show_raw;
        let mut mode = None;
        egui::Window::new("Open raw image")
            .open(&mut visible)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Architecture");
                    ui.selectable_value(&mut self.raw_arch, Architecture::X86_64, "x86-64");
                    ui.selectable_value(&mut self.raw_arch, Architecture::X86, "x86");
                });
                ui.horizontal(|ui| {
                    ui.label("Base address");
                    ui.text_edit_singleline(&mut self.raw_base);
                });
                if ui.button("Choose raw file…").clicked() {
                    match parse_address(&self.raw_base) {
                        Ok(base) => {
                            mode = Some(LoadMode::Raw {
                                base,
                                architecture: self.raw_arch,
                            })
                        }
                        Err(error) => self.error(error),
                    }
                }
            });
        self.show_raw = visible && mode.is_none();
        if let Some(mode) = mode {
            self.choose(ctx, mode);
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(theme::STORAGE_KEY, self.theme.id().into());
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_workers(ctx);
        if ctx.input(|input| input.viewport().close_requested()) && !self.confirm_exit() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::O)) {
            self.choose(ctx, LoadMode::Auto);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::S)) {
            self.save(false);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::W)) {
            self.close_tab(ctx, self.version);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Tab))
            && let Some((&id, _)) = self
                .background
                .range(self.version..)
                .next()
                .or_else(|| self.background.first_key_value())
        {
            self.activate(ctx, id);
        }
        if ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::F)
        }) && self.session.is_some()
        {
            self.state.view = View::Search;
        } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::F))
            && self.session.is_some()
        {
            self.state.find.open();
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::D))
            && let Some(address) = self.state.selected_address
        {
            let exists = self
                .session
                .as_ref()
                .is_some_and(|session| session.project.bookmark(address).is_some());
            let name = if exists {
                String::new()
            } else {
                format!("Bookmark 0x{address:X}")
            };
            self.bookmark(address, name);
        }
        let dropped = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });
        self.enqueue_files(ctx, dropped, LoadMode::Auto);
        self.top(ctx);
        self.document_tabs(ctx);
        self.status(ctx);
        let palette = self.theme.palette();
        if self.session.is_some() {
            egui::SidePanel::left("navigation")
                .resizable(true)
                .default_width(238.0)
                .width_range(175.0..=490.0)
                .frame(theme::frame(palette.panel, 11, 11))
                .show(ctx, |ui| {
                    ui.push_id(self.version, |ui| self.navigator(ui));
                });
            if self.show_inspector {
                egui::SidePanel::right("inspector")
                    .resizable(true)
                    .default_width(248.0)
                    .width_range(205.0..=490.0)
                    .frame(theme::frame(palette.panel, 12, 11))
                    .show(ctx, |ui| {
                        ui.push_id(self.version, |ui| self.inspector(ui));
                    });
            }
            egui::CentralPanel::default()
                .frame(theme::frame(palette.bg, 12, 10))
                .show(ctx, |ui| {
                    ui.push_id(self.version, |ui| self.workspace(ui));
                });
        } else {
            egui::CentralPanel::default()
                .frame(theme::frame(palette.bg, 20, 20))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.29).max(35.0));
                        if let Some(mark) = &self.mark {
                            ui.image((mark.id(), egui::vec2(128.0, 128.0)));
                        }
                        ui.add_space(12.0);
                        ui.label(RichText::new("Open a binary").size(32.0).strong());
                        ui.label(
                            RichText::new("Drop a binary or Hexaris project here to inspect it.")
                                .color(palette.muted),
                        );
                        ui.add_space(15.0);
                        if ui
                            .add_enabled(
                                self.opening.is_none(),
                                icon_button(&self.open_icon, "Choose file…", palette.text),
                            )
                            .clicked()
                        {
                            self.choose(ctx, LoadMode::Auto);
                        }
                        if ui
                            .add_enabled(
                                self.opening.is_none(),
                                Button::new("Open raw image…").frame(false),
                            )
                            .clicked()
                        {
                            self.show_raw = true;
                        }
                    });
                });
        }
        if self.show_raw {
            self.raw_window(ctx);
        }
        self.patch_window(ctx);
        self.rename_window(ctx);
        self.values.show(ctx);
        self.tools_window(ctx);
    }
}

fn icon_button(
    texture: &Option<TextureHandle>,
    label: &'static str,
    ink: egui::Color32,
) -> Button<'static> {
    match texture {
        Some(texture) => Button::image_and_text(
            egui::Image::from_texture((texture.id(), egui::vec2(16.0, 16.0))).tint(ink),
            label,
        ),
        None => Button::new(label),
    }
}

fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("hxproj"))
}

fn load(path: PathBuf, mode: LoadMode) -> hexaris_engine::Result<Session> {
    let path = path.canonicalize().unwrap_or(path);
    let is_project = mode == LoadMode::Auto && is_project(&path);
    let project = if is_project {
        Project::open(&path)?
    } else {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Project::new(name, Image::open(&path, mode)?)
    };
    let analysis = analyze(project.image());
    let references = ReferenceIndex::build(&analysis);
    let triage = triage(project.image());
    Ok(Session {
        source: path.clone(),
        load_mode: mode,
        project,
        analysis,
        references,
        triage,
        path: is_project.then_some(path),
        dirty: false,
        lifted: BTreeMap::new(),
        pseudocode: BTreeMap::new(),
        patches: Vec::new(),
    })
}

fn label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(theme::current(ui.ctx()).muted));
}
