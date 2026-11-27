use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use eframe::egui::{self, Button, Context, Key, RichText, ScrollArea, TextureHandle, Ui};
use hexaris_engine::{
    Analysis, Architecture, BytePatch, BytePattern, Decompilation, Function, Image, LiftedFunction,
    LinearListing, LoadMode, Project, StringItem, analyze, analyze_function, decompile,
    disassemble, lift, parse_address,
};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use crate::theme::{self, ACCENT, BG, MUTED, PANEL, WARN};

mod views;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum View {
    #[default]
    Disassembly,
    Linear,
    Flow,
    Registers,
    Pseudocode,
    Hex,
    Strings,
    Search,
}

impl View {
    const ALL: [Self; 8] = [
        Self::Disassembly,
        Self::Linear,
        Self::Flow,
        Self::Registers,
        Self::Pseudocode,
        Self::Hex,
        Self::Strings,
        Self::Search,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Disassembly => "Disassembly",
            Self::Linear => "Linear",
            Self::Flow => "Control flow",
            Self::Registers => "Register IR",
            Self::Pseudocode => "Pseudo-C",
            Self::Hex => "Hex",
            Self::Strings => "Strings",
            Self::Search => "Byte search",
        }
    }
}

struct Session {
    project: Project,
    analysis: Analysis,
    path: Option<PathBuf>,
    dirty: bool,
    lifted: BTreeMap<u64, LiftedFunction>,
    pseudocode: BTreeMap<u64, Decompilation>,
}

#[derive(Default)]
struct PatchDialog {
    offset: String,
    expected: String,
    replacement: String,
}

impl Session {
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

pub struct App {
    session: Option<Session>,
    opening: Option<Receiver<hexaris_engine::Result<Session>>>,
    version: u64,
    title: String,
    message: Option<(bool, String)>,
    view: View,
    filter: String,
    browser_tab: usize,
    selected_function: Option<u64>,
    selected_address: Option<u64>,
    hex_jump: Option<usize>,
    disassembly_jump: Option<usize>,
    linear: Option<LinearListing>,
    jump_text: String,
    note: String,
    name: String,
    query: String,
    strings_min: usize,
    strings: Option<Vec<StringItem>>,
    hits: Option<Vec<usize>>,
    querying: Option<Receiver<QueryResult>>,
    show_inspector: bool,
    show_raw: bool,
    patch_dialog: Option<PatchDialog>,
    raw_base: String,
    raw_arch: Architecture,
    mark: Option<TextureHandle>,
    open_icon: Option<TextureHandle>,
    save_icon: Option<TextureHandle>,
    search_icon: Option<TextureHandle>,
    inspector_icon: Option<TextureHandle>,
    navigation_back: Vec<(u64, u64)>,
    navigation_forward: Vec<(u64, u64)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, input: Option<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        theme::install(ctx);
        let mut app = Self {
            session: None,
            opening: None,
            version: 0,
            title: "Hexaris".into(),
            message: None,
            view: View::default(),
            filter: String::new(),
            browser_tab: 0,
            selected_function: None,
            selected_address: None,
            hex_jump: None,
            disassembly_jump: None,
            linear: None,
            jump_text: String::new(),
            note: String::new(),
            name: String::new(),
            query: String::new(),
            strings_min: 5,
            strings: None,
            hits: None,
            querying: None,
            show_inspector: true,
            show_raw: false,
            patch_dialog: None,
            raw_base: "0x1000".into(),
            raw_arch: Architecture::X86_64,
            mark: theme::texture(ctx, "hexaris-mark", theme::MARK, 128),
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
            navigation_back: Vec::new(),
            navigation_forward: Vec::new(),
        };
        if let Some(path) = input {
            app.start_open(ctx, path, LoadMode::Auto);
        }
        app
    }

    fn error(&mut self, message: impl ToString) {
        self.message = Some((true, message.to_string()));
    }

    fn confirm_replace(&self) -> bool {
        !self.session.as_ref().is_some_and(|session| session.dirty)
            || MessageDialog::new()
                .set_title("Unsaved analysis changes")
                .set_description("Discard changes to the current project?")
                .set_buttons(MessageButtons::YesNo)
                .set_level(MessageLevel::Warning)
                .show()
                == MessageDialogResult::Yes
    }

    fn choose(&mut self, ctx: &Context, mode: LoadMode) {
        if self.opening.is_some() {
            return;
        }
        if let Some(path) = FileDialog::new()
            .set_title("Open a binary or Hexaris project")
            .pick_file()
        {
            self.start_open(ctx, path, mode);
        }
    }

    fn start_open(&mut self, ctx: &Context, path: PathBuf, mode: LoadMode) {
        if self.opening.is_some() || !self.confirm_replace() {
            return;
        }
        self.message = None;
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
                        Ok(session) => {
                            let entry =
                                session.project.image().info.entry.filter(|address| {
                                    session.analysis.functions.contains_key(address)
                                });
                            let address =
                                entry.or_else(|| session.analysis.functions.keys().next().copied());
                            self.title = format!("{}  |  Hexaris", session.project.name());
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title.clone()));
                            self.session = Some(session);
                            self.version += 1;
                            self.view = if address.is_some() {
                                View::Disassembly
                            } else {
                                View::Hex
                            };
                            self.browser_tab = 0;
                            self.filter.clear();
                            self.query.clear();
                            self.jump_text.clear();
                            self.linear = None;
                            self.patch_dialog = None;
                            self.hits = None;
                            self.strings = None;
                            self.querying = None;
                            self.navigation_back.clear();
                            self.navigation_forward.clear();
                            self.hex_jump = Some(0);
                            self.select(address, address);
                        }
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
        if let Some(worker) = &self.querying {
            match worker.try_recv() {
                Ok(QueryResult::Bytes(hits)) => {
                    self.hits = Some(hits);
                    self.querying = None;
                }
                Ok(QueryResult::Strings(items)) => {
                    self.strings = Some(items);
                    self.querying = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.querying = None;
                    self.error("The search exited without a result.");
                }
                Err(TryRecvError::Empty) => {}
            }
        }
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
                session.path = Some(path.clone());
                session.dirty = false;
                self.message = Some((false, format!("Saved {}", path.display())));
            }
            Err(error) => self.error(error),
        }
    }

    fn select(&mut self, function: Option<u64>, address: Option<u64>) {
        self.selected_function = function;
        self.selected_address = address;
        let session = self.session.as_ref();
        self.note = address
            .and_then(|address| session?.project.comment(address))
            .unwrap_or_default()
            .into();
        self.name = function
            .and_then(|address| session?.project.function_name(address))
            .unwrap_or_default()
            .into();
        self.disassembly_jump = function.and_then(|function| {
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
            .selected_function
            .zip(self.selected_address)
            .filter(|&before| before != (function, address))
        {
            self.navigation_back.push(before);
            self.navigation_forward.clear();
        }
        self.select(Some(function), Some(address));
    }

    fn history(&mut self, forward: bool) {
        let (from, to) = if forward {
            (&mut self.navigation_forward, &mut self.navigation_back)
        } else {
            (&mut self.navigation_back, &mut self.navigation_forward)
        };
        if let Some((function, address)) = from.pop() {
            to.extend(self.selected_function.zip(self.selected_address));
            self.select(Some(function), Some(address));
            self.view = View::Disassembly;
        }
    }

    fn goto(&mut self, address: u64) {
        let Some(session) = &self.session else { return };
        let functions = &session.analysis.functions;
        let owner = if functions.contains_key(&address) {
            Some(address)
        } else {
            functions
                .iter()
                .find(|(_, function)| {
                    function
                        .instructions
                        .binary_search_by_key(&address, |i| i.address)
                        .is_ok()
                })
                .map(|(&address, _)| address)
        };
        if let Some(function) = owner {
            self.navigate(function, address);
            self.view = View::Disassembly;
        } else if let Some(offset) = session.project.image().file_offset(address) {
            self.show_hex(offset);
        } else {
            self.error(format!("0x{address:X} is not backed by file bytes."));
        }
    }

    fn show_hex(&mut self, offset: usize) {
        let address = self
            .session
            .as_ref()
            .and_then(|s| s.project.image().virtual_address(offset));
        self.select(self.selected_function, address);
        self.hex_jump = Some(offset);
        self.view = View::Hex;
    }

    fn analyze_at(&mut self, address: u64) {
        let Some(session) = &mut self.session else {
            return;
        };
        match analyze_function(session.project.image(), address) {
            Ok(function) => {
                session.analysis.functions.insert(address, function);
                session.lifted.remove(&address);
                session.pseudocode.remove(&address);
                self.navigate(address, address);
                self.view = View::Disassembly;
            }
            Err(error) => self.error(error),
        }
    }

    fn decode_linear(&mut self, address: u64) {
        let Some(session) = &self.session else { return };
        match disassemble(session.project.image(), address, 512) {
            Ok(listing) => {
                self.linear = Some(listing);
                self.select(self.selected_function, Some(address));
                self.view = View::Linear;
            }
            Err(error) => self.error(error),
        }
    }

    fn query(&mut self, ctx: &Context, strings: bool) {
        let Some(session) = &self.session else { return };
        if self.querying.is_some() {
            return;
        }
        let pattern = if strings {
            None
        } else {
            match BytePattern::parse(&self.query) {
                Ok(pattern) => Some(pattern),
                Err(error) => {
                    self.error(error);
                    return;
                }
            }
        };
        let bytes = session.project.image().shared_bytes();
        let minimum = self.strings_min;
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.querying = Some(rx);
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
        egui::TopBottomPanel::top("application_header")
            .frame(theme::frame(PANEL, 13, 9))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(mark) = &self.mark {
                        ui.image((mark.id(), egui::vec2(29.0, 29.0)));
                    }
                    ui.label(RichText::new("Hexaris").size(21.0).strong());
                    ui.add_space(13.0);
                    if ui
                        .add_enabled(self.opening.is_none(), icon_button(&self.open_icon, "Open"))
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
                                Button::new("Export byte patch…"),
                            )
                            .clicked()
                        {
                            let offset = self.selected_address.and_then(|address| {
                                self.session.as_ref()?.project.image().file_offset(address)
                            });
                            self.patch_dialog = Some(PatchDialog {
                                offset: offset
                                    .map(|offset| format!("0x{offset:X}"))
                                    .unwrap_or_default(),
                                ..Default::default()
                            });
                            ui.close();
                        }
                    });
                    if ui
                        .add_enabled(
                            self.session.is_some() && self.opening.is_none(),
                            icon_button(&self.save_icon, "Save project"),
                        )
                        .on_hover_text("Save project (Ctrl+S)")
                        .clicked()
                    {
                        self.save(false);
                    }
                    if let Some(session) = &self.session {
                        ui.separator();
                        ui.add(
                            egui::Label::new(RichText::new(session.project.name()).color(MUTED))
                                .truncate(),
                        );
                        if session.dirty {
                            ui.label(RichText::new("•").color(ACCENT));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                self.session.is_some(),
                                icon_button(&self.inspector_icon, "Inspector")
                                    .selected(self.show_inspector && self.session.is_some()),
                            )
                            .clicked()
                        {
                            self.show_inspector = !self.show_inspector;
                        }
                        if ui
                            .add_enabled(
                                self.session.is_some(),
                                icon_button(&self.search_icon, "Find bytes"),
                            )
                            .clicked()
                        {
                            self.view = View::Search;
                        }
                    });
                });
            });
    }

    fn status(&mut self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(theme::frame(PANEL, 13, 5))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.opening.is_some() {
                        ui.spinner();
                        ui.label("Opening and analyzing…");
                    } else if let Some((error, message)) = &self.message {
                        ui.add(
                            egui::Label::new(RichText::new(message).color(if *error {
                                WARN
                            } else {
                                ACCENT
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
                            .color(MUTED),
                        );
                    } else {
                        ui.label(RichText::new("Ready to open a local file").color(MUTED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(address) = self.selected_address {
                            ui.label(
                                RichText::new(format!("0x{address:X}"))
                                    .monospace()
                                    .color(MUTED),
                            );
                        }
                    });
                });
            });
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

    fn patch_window(&mut self, ctx: &Context) {
        let Some(dialog) = &mut self.patch_dialog else {
            return;
        };
        let mut open = true;
        let mut write = false;
        egui::Window::new("Export byte patch")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                label(
                    ui,
                    "Write a new binary with a fixed-length byte replacement.",
                );
                egui::Grid::new("byte-patch-fields")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("File offset");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.offset)
                                .font(egui::TextStyle::Monospace),
                        );
                        ui.end_row();
                        ui.label("Expected bytes");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.expected)
                                .font(egui::TextStyle::Monospace)
                                .hint_text("48 8B"),
                        );
                        ui.end_row();
                        ui.label("Replacement bytes");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.replacement)
                                .font(egui::TextStyle::Monospace),
                        );
                        ui.end_row();
                    });
                write = ui
                    .add_enabled(self.opening.is_none(), Button::new("Export copy…"))
                    .clicked();
            });
        if !open {
            self.patch_dialog = None;
        }
        if write && let Some(dialog) = &self.patch_dialog {
            let offset = match parse_address(&dialog.offset)
                .ok()
                .and_then(|value| usize::try_from(value).ok())
            {
                Some(offset) => offset,
                None => {
                    self.error("Enter a valid file offset.");
                    return;
                }
            };
            match BytePatch::parse(offset, &dialog.expected, &dialog.replacement) {
                Ok(patch) => self.export_patch(patch),
                Err(error) => self.error(error),
            }
        }
    }

    fn export_patch(&mut self, patch: BytePatch) {
        let Some(session) = &self.session else { return };
        let Some(path) = FileDialog::new()
            .set_title("Export patched binary copy")
            .save_file()
        else {
            return;
        };
        match patch.write_copy(session.project.image(), &path) {
            Ok(()) => {
                self.patch_dialog = None;
                self.message = Some((false, format!("Exported {}", path.display())));
            }
            Err(error) => self.error(error),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_workers(ctx);
        if ctx.input(|input| input.viewport().close_requested()) && !self.confirm_replace() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::O)) {
            self.choose(ctx, LoadMode::Auto);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::S)) {
            self.save(false);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::F))
            && self.session.is_some()
        {
            self.view = View::Search;
        }
        if let Some(path) = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .find_map(|file| file.path.clone())
        }) {
            self.start_open(ctx, path, LoadMode::Auto);
        }
        self.top(ctx);
        self.status(ctx);
        if self.session.is_some() {
            egui::SidePanel::left("navigation")
                .resizable(true)
                .default_width(238.0)
                .width_range(175.0..=490.0)
                .frame(theme::frame(PANEL, 11, 11))
                .show(ctx, |ui| self.navigator(ui));
            if self.show_inspector {
                egui::SidePanel::right("inspector")
                    .resizable(true)
                    .default_width(248.0)
                    .width_range(205.0..=490.0)
                    .frame(theme::frame(PANEL, 12, 11))
                    .show(ctx, |ui| self.inspector(ui));
            }
            egui::CentralPanel::default()
                .frame(theme::frame(BG, 12, 10))
                .show(ctx, |ui| self.workspace(ui));
        } else {
            egui::CentralPanel::default()
                .frame(theme::frame(BG, 20, 20))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.29).max(35.0));
                        if let Some(mark) = &self.mark {
                            ui.image((mark.id(), egui::vec2(86.0, 86.0)));
                        }
                        ui.add_space(12.0);
                        ui.label(RichText::new("Open a binary").size(32.0).strong());
                        ui.label(
                            RichText::new("Drop a binary or Hexaris project here to inspect it.")
                                .color(MUTED),
                        );
                        ui.add_space(15.0);
                        if ui
                            .add_enabled(
                                self.opening.is_none(),
                                icon_button(&self.open_icon, "Choose file…"),
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
    }
}

fn icon_button(texture: &Option<TextureHandle>, label: &'static str) -> Button<'static> {
    match texture {
        Some(texture) => Button::image_and_text((texture.id(), egui::vec2(16.0, 16.0)), label),
        None => Button::new(label),
    }
}

fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("hxproj"))
}

fn load(path: PathBuf, mode: LoadMode) -> hexaris_engine::Result<Session> {
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
    Ok(Session {
        project,
        analysis,
        path: is_project.then_some(path),
        dirty: false,
        lifted: BTreeMap::new(),
        pseudocode: BTreeMap::new(),
    })
}

fn label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(MUTED));
}
