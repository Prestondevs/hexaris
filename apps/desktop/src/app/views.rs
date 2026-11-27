use std::{collections::BTreeMap, fmt::Write as _};

use eframe::egui::{self, Align2, FontId, Grid, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use hexaris_engine::{CallSite, EdgeKind, Image, Reference, Region};

use super::*;

const ROW: f32 = 22.0;

impl App {
    pub(super) fn navigator(&mut self, ui: &mut Ui) {
        let palette = self.theme.palette();
        let Some(session) = &self.session else { return };
        ui.heading("Image browser");
        let info = &session.project.image().info;
        ui.label(
            RichText::new(format!("{}  /  {}", info.format, info.architecture.name()))
                .color(palette.accent),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().button_padding.x = 7.0;
            ui.selectable_value(&mut self.state.browser_tab, 0, "Functions");
            ui.selectable_value(&mut self.state.browser_tab, 1, "Regions");
            ui.selectable_value(&mut self.state.browser_tab, 2, "Symbols");
            ui.selectable_value(&mut self.state.browser_tab, 3, "Imports");
            ui.selectable_value(&mut self.state.browser_tab, 4, "Exports");
            ui.selectable_value(&mut self.state.browser_tab, 5, "Bookmarks");
        });
        ui.separator();
        let mut target = None;
        let mut offset = None;
        let mut context_target = None;
        match self.state.browser_tab {
            0 => {
                ui.add(
                    egui::TextEdit::singleline(&mut self.state.filter)
                        .hint_text("Filter functions…")
                        .desired_width(f32::INFINITY),
                );
                label(
                    ui,
                    &format!("{} discovered", session.analysis.functions.len()),
                );
                let filter = self.state.filter.to_lowercase();
                let rows: Vec<_> = session
                    .analysis
                    .functions
                    .values()
                    .filter(|function| {
                        filter.is_empty()
                            || session
                                .function_name(function)
                                .to_lowercase()
                                .contains(&filter)
                            || format!("0x{:x}", function.address).contains(&filter)
                    })
                    .collect();
                ScrollArea::vertical()
                    .id_salt(("functions", self.version))
                    .auto_shrink([false; 2])
                    .show_rows(ui, ROW, rows.len(), |ui, range| {
                        for &function in &rows[range] {
                            let text = format!(
                                "{:X}   {}",
                                function.address,
                                session.function_name(function)
                            );
                            let button = Button::selectable(
                                self.state.selected_function == Some(function.address),
                                RichText::new(text).monospace(),
                            )
                            .truncate()
                            .min_size(vec2(ui.available_width(), ROW));
                            let response = ui.add(button);
                            if response.clicked() {
                                target = Some(function.address);
                            }
                            response.context_menu(|ui| {
                                if ui.button("Function context…").clicked() {
                                    context_target = Some(function.address);
                                    ui.close();
                                }
                            });
                        }
                    });
            }
            1 => {
                ScrollArea::vertical()
                    .id_salt(("regions", self.version))
                    .show(ui, |ui| {
                        if let Some(entry) = info.entry
                            && ui.link(format!("Entry 0x{entry:X}")).clicked()
                        {
                            target = Some(entry);
                        }
                        if let Some(base) = info.image_base {
                            label(ui, &format!("Base 0x{base:X}"));
                        }
                        for (label, regions) in
                            [("Sections", &info.sections), ("Segments", &info.segments)]
                        {
                            if regions.is_empty() {
                                continue;
                            }
                            ui.add_space(7.0);
                            ui.strong(label);
                            for region in regions {
                                if region_row(ui, region) {
                                    offset = region.file_offset.map(|value| value as usize);
                                }
                            }
                        }
                    });
            }
            2 => {
                ui.label(format!("{} defined symbols", info.symbols.len()));
                ScrollArea::vertical()
                    .id_salt(("symbols", self.version))
                    .show_rows(ui, ROW, info.symbols.len(), |ui, range| {
                        for symbol in &info.symbols[range] {
                            let button = Button::new(
                                RichText::new(format!("{:X}   {}", symbol.address, symbol.name))
                                    .monospace(),
                            )
                            .truncate();
                            if ui.add_enabled(!info.is_relocatable, button).clicked() {
                                target = Some(symbol.address);
                            }
                        }
                    });
            }
            3 => {
                label(ui, &format!("{} named imports", info.imports.len()));
                ScrollArea::vertical()
                    .id_salt(("imports", self.version))
                    .show_rows(ui, ROW, info.imports.len(), |ui, range| {
                        for import in &info.imports[range] {
                            let name = if import.library.is_empty() {
                                import.name.clone()
                            } else {
                                format!("{}!{}", import.library, import.name)
                            };
                            ui.add(egui::Label::new(RichText::new(&name).monospace()).truncate())
                                .on_hover_text(name);
                        }
                    });
            }
            4 => {
                label(ui, &format!("{} named exports", info.exports.len()));
                ScrollArea::vertical()
                    .id_salt(("exports", self.version))
                    .show_rows(ui, ROW, info.exports.len(), |ui, range| {
                        for export in &info.exports[range] {
                            let text = format!("{:X}   {}", export.address, export.name);
                            let button = Button::new(RichText::new(&text).monospace()).truncate();
                            if ui
                                .add_enabled(!info.is_relocatable, button)
                                .on_hover_text(text)
                                .clicked()
                            {
                                target = Some(export.address);
                            }
                        }
                    });
            }
            _ => {
                let rows: Vec<_> = session.project.bookmarks().collect();
                label(ui, &format!("{} saved bookmarks", rows.len()));
                if rows.is_empty() {
                    label(ui, "Select an address and press Ctrl+D to bookmark it.");
                }
                ScrollArea::vertical()
                    .id_salt(("bookmarks", self.version))
                    .show_rows(ui, ROW, rows.len(), |ui, range| {
                        for &(address, name) in &rows[range] {
                            let text = format!("{address:X}   {name}");
                            if ui
                                .add(Button::new(RichText::new(&text).monospace()).truncate())
                                .on_hover_text(text)
                                .clicked()
                            {
                                target = Some(address);
                            }
                        }
                    });
            }
        }
        if let Some(address) = target {
            self.goto(address);
        }
        if let Some(offset) = offset {
            self.show_hex(offset);
        }
        if let Some(address) = context_target {
            self.open_context(address);
        }
    }

    pub(super) fn workspace(&mut self, ui: &mut Ui) {
        let palette = self.theme.palette();
        let mut nav = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.state.navigation_back.is_empty(), Button::new("Back"))
                .on_hover_text("Back")
                .clicked()
            {
                self.history(false);
            }
            if ui
                .add_enabled(
                    !self.state.navigation_forward.is_empty(),
                    Button::new("Forward"),
                )
                .on_hover_text("Forward")
                .clicked()
            {
                self.history(true);
            }
            let address_input = ui.add(
                egui::TextEdit::singleline(&mut self.state.jump_text)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("Virtual address")
                    .desired_width(134.0),
            );
            if ui.button("Go").clicked()
                || (address_input.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)))
            {
                nav = Some(false);
            }
            if ui
                .button("Analyze here")
                .on_hover_text("Decode a new function at the entered virtual address")
                .clicked()
            {
                nav = Some(true);
            }
        });
        if let Some(analyze) = nav {
            match parse_address(&self.state.jump_text) {
                Ok(address) if analyze => self.analyze_at(address),
                Ok(address) if self.state.view == View::Linear => self.decode_linear(address),
                Ok(address) => self.goto(address),
                Err(_) => {
                    self.error("Enter a decimal address or prefix a hexadecimal address with 0x.")
                }
            }
        }
        ui.add_space(5.0);
        let previous = self.state.view;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().button_padding.x = 7.0;
            for view in View::ALL {
                ui.selectable_value(&mut self.state.view, view, view.label());
            }
        });
        if self.state.view == View::Linear
            && previous != View::Linear
            && let Some(address) = self.state.selected_address
        {
            self.decode_linear(address);
        }
        ui.separator();
        if let Some(session) = &self.session {
            for notice in &session.analysis.notices {
                ui.label(RichText::new(notice).color(palette.warn));
            }
            for warning in &session.project.image().info.metadata_warnings {
                ui.label(RichText::new(warning).color(palette.warn));
            }
        }
        match self.state.view {
            View::Overview => self.overview(ui),
            View::Calls => self.calls(ui),
            View::Disassembly | View::Flow | View::Registers | View::Pseudocode => {
                self.function_view(ui)
            }
            View::Linear => self.linear_view(ui),
            View::Hex => self.hex(ui),
            View::Strings => self.results(ui, true),
            View::Search => self.results(ui, false),
        }
    }

    fn calls(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        let Some(address) = self.state.selected_function else {
            label(ui, "Select a function to inspect direct calls.");
            return;
        };
        if let Some(function) = session.analysis.functions.get(&address) {
            ui.heading(session.function_name(function));
        }
        label(
            ui,
            "Select a function or call site to open it in disassembly.",
        );
        let selection = ui.columns(2, |columns| {
            let caller = call_rows(&mut columns[0], self.version, session, address, true);
            let callee = call_rows(&mut columns[1], self.version, session, address, false);
            caller.or(callee)
        });
        match selection {
            Some(CallSelection::Site(caller, site)) => {
                self.navigate(caller, site);
                self.state.view = View::Disassembly;
            }
            Some(CallSelection::Function(target)) => self.goto(target),
            None => {}
        }
    }

    fn overview(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        let info = &session.triage;
        let mut address = None;
        let mut offset = None;
        ScrollArea::both()
            .id_salt(("image_overview", self.version))
            .show(ui, |ui| {
                ui.heading("Image overview");
                Grid::new("image-metadata")
                    .spacing([16.0, 5.0])
                    .show(ui, |ui| {
                        label(ui, "Format");
                        ui.label(format!(
                            "{} · {}-bit {} endian",
                            info.format,
                            info.bits,
                            if info.little_endian { "little" } else { "big" }
                        ));
                        ui.end_row();
                        label(ui, "Architecture");
                        ui.label(info.architecture.name());
                        ui.end_row();
                        label(ui, "Image base");
                        ui.monospace(
                            info.image_base
                                .map_or_else(|| "—".into(), |base| format!("0x{base:X}")),
                        );
                        ui.end_row();
                        label(ui, "Entry point");
                        if let Some(entry) = info.entry {
                            if ui.link(format!("0x{entry:X}")).clicked() {
                                address = Some(entry);
                            }
                        } else {
                            label(ui, "—");
                        }
                        ui.end_row();
                        label(ui, "Size");
                        ui.label(format!("{} bytes", info.size_bytes));
                        ui.end_row();
                        label(ui, "Sections");
                        ui.label(format!(
                            "{} total · {} executable",
                            info.section_count, info.executable_section_count
                        ));
                        ui.end_row();
                        label(ui, "Names");
                        ui.label(format!(
                            "{} symbols · {} imports · {} exports",
                            info.symbol_count, info.named_import_count, info.named_export_count
                        ));
                        ui.end_row();
                    });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.strong("SHA-256");
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(info.sha256.clone());
                    }
                });
                ui.monospace(&info.sha256);
                ui.add_space(13.0);
                ui.horizontal_wrapped(|ui| {
                    ui.heading("File entropy");
                    label(
                        ui,
                        &format!("{:.3} bits per byte overall", info.entropy_bits_per_byte),
                    );
                });
                offset = entropy_chart(ui, info);
            });
        if let Some(address) = address {
            self.goto(address);
        }
        if let Some(offset) = offset {
            self.show_hex(offset);
        }
    }

    fn linear_view(&mut self, ui: &mut Ui) {
        let palette = self.theme.palette();
        let Some(listing) = &self.state.linear else {
            label(
                ui,
                "Enter a virtual address to decode a range of executable bytes.",
            );
            return;
        };
        let mut selected = None;
        let mut next = None;
        ui.horizontal_wrapped(|ui| {
            ui.heading(format!("Sequential decode from 0x{:X}", listing.start));
            if let Some(address) = listing.next_address
                && ui.button("Next 512").clicked()
            {
                next = Some(address);
            }
        });
        if let Some(address) = listing.invalid_at {
            ui.label(
                RichText::new(format!(
                    "Stopped at an invalid or truncated instruction at 0x{address:X}"
                ))
                .color(palette.warn),
            );
        }
        ScrollArea::both()
            .auto_shrink([false; 2])
            .id_salt(("linear", self.version, listing.start))
            .show_rows(ui, ROW, listing.instructions.len(), |ui, range| {
                for instruction in &listing.instructions[range] {
                    let text = format!(
                        "{:016X}   {:26}  {}",
                        instruction.address,
                        hex_bytes(&instruction.bytes),
                        instruction.text
                    );
                    if ui
                        .add(
                            Button::selectable(
                                self.state.selected_address == Some(instruction.address),
                                RichText::new(text).monospace(),
                            )
                            .min_size(vec2(ui.available_width(), ROW)),
                        )
                        .clicked()
                    {
                        selected = Some(instruction.address);
                    }
                }
            });
        if let Some(address) = selected {
            self.select(self.state.selected_function, Some(address));
        }
        if let Some(address) = next {
            self.decode_linear(address);
        }
    }

    fn function_view(&mut self, ui: &mut Ui) {
        let palette = self.theme.palette();
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(function) = self
            .state
            .selected_function
            .and_then(|address| session.analysis.functions.get(&address))
        else {
            label(
                ui,
                "Select a discovered function or analyze a virtual address.",
            );
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.heading(session.function_name(function));
            ui.label(
                RichText::new(format!("0x{:X}", function.address))
                    .monospace()
                    .color(palette.accent),
            );
            label(
                ui,
                &format!(
                    "{} instructions  ·  {} blocks",
                    function.instructions.len(),
                    function.blocks.len()
                ),
            );
        });
        if !function.issues.is_empty() {
            ui.collapsing(format!("{} analysis notes", function.issues.len()), |ui| {
                for issue in function.issues.iter().take(30) {
                    ui.label(RichText::new(issue).color(palette.warn));
                }
            });
        }
        let mut target = None;
        match self.state.view {
            View::Disassembly => {
                let mut area = ScrollArea::both()
                    .id_salt(("disassembly", self.version, function.address))
                    .auto_shrink([false; 2]);
                if let Some(row) = self.state.disassembly_jump.take() {
                    area = area.vertical_scroll_offset(
                        (ROW + ui.spacing().item_spacing.y) * row.saturating_sub(3) as f32,
                    );
                }
                area.show_rows(ui, ROW, function.instructions.len(), |ui, range| {
                    for instruction in &function.instructions[range] {
                        let bytes = hex_bytes(&instruction.bytes);
                        let comment = session
                            .project
                            .comment(instruction.address)
                            .unwrap_or_default()
                            .lines()
                            .next()
                            .unwrap_or_default();
                        let text = format!(
                            "{:016X}   {:26}  {}{}",
                            instruction.address,
                            bytes,
                            instruction.text,
                            if comment.is_empty() {
                                String::new()
                            } else {
                                format!("    ; {comment}")
                            }
                        );
                        let button = Button::selectable(
                            self.state.selected_address == Some(instruction.address),
                            RichText::new(text).monospace(),
                        )
                        .min_size(vec2(ui.available_width(), ROW));
                        let response = ui.add(button);
                        if response.clicked() {
                            target = Some(instruction.address);
                        }
                        response.context_menu(|ui| {
                            if ui.button("Copy virtual address").clicked() {
                                ui.ctx().copy_text(format!("0x{:X}", instruction.address));
                                ui.close();
                            }
                            if ui.button("Copy instruction").clicked() {
                                ui.ctx().copy_text(instruction.text.clone());
                                ui.close();
                            }
                        });
                    }
                });
            }
            View::Flow => {
                if let Some(address) =
                    graph(ui, self.version, function, self.state.selected_address)
                {
                    target = Some(address);
                    self.state.view = View::Disassembly;
                }
            }
            View::Registers => {
                label(
                    ui,
                    "32-bit register SSA over discovered blocks. Opaque instructions invalidate register values.",
                );
                let lifted = session
                    .lifted
                    .entry(function.address)
                    .or_insert_with(|| lift(function));
                ScrollArea::both()
                    .id_salt(("register_ir", self.version, function.address))
                    .show(ui, |ui| {
                        if let Some(pseudocode) = &lifted.register_pseudocode {
                            ui.collapsing("Register pseudocode (EAX return)", |ui| {
                                ui.monospace(pseudocode);
                            });
                        }
                        for block in &lifted.blocks {
                            ui.label(
                                RichText::new(format!("Block 0x{:X}", block.address))
                                    .color(palette.accent)
                                    .monospace(),
                            );
                            if !block.inputs.is_empty() {
                                ui.push_id(block.address, |ui| {
                                    ui.collapsing("Incoming register values", |ui| {
                                        for input in &block.inputs {
                                            let sources = input
                                                .incoming
                                                .iter()
                                                .map(|source| {
                                                    let origin = source.predecessor.map_or_else(
                                                        || "entry".into(),
                                                        |address| format!("0x{address:X}"),
                                                    );
                                                    format!("{origin} → {}", source.value)
                                                })
                                                .collect::<Vec<_>>()
                                                .join(", ");
                                            ui.monospace(format!("{} = φ({sources})", input.name));
                                        }
                                    });
                                });
                            }
                            for statement in &block.statements {
                                let text = RichText::new(format!(
                                    "{:016X}   {}",
                                    statement.address, statement.text
                                ))
                                .monospace()
                                .color(if statement.supported {
                                    palette.text
                                } else {
                                    palette.muted
                                });
                                if ui
                                    .selectable_label(
                                        self.state.selected_address == Some(statement.address),
                                        text,
                                    )
                                    .clicked()
                                {
                                    target = Some(statement.address);
                                }
                            }
                        }
                    });
            }
            View::Pseudocode => {
                let result = session
                    .pseudocode
                    .entry(function.address)
                    .or_insert_with(|| decompile(session.project.image(), function));
                if let Some(code) = &result.code {
                    label(
                        ui,
                        "Windows x64 leaf-function projection · unsigned 32-bit values",
                    );
                    ScrollArea::both()
                        .id_salt(("pseudocode", self.version, function.address))
                        .show(ui, |ui| {
                            if ui.button("Copy pseudocode").clicked() {
                                ui.ctx().copy_text(code.clone());
                            }
                            ui.monospace(code);
                        });
                } else if let Some(reason) = &result.unavailable {
                    ui.label(RichText::new(reason).color(palette.warn));
                }
            }
            _ => {}
        }
        let function_address = function.address;
        if let Some(address) = target {
            self.navigate(function_address, address);
        }
    }

    pub(super) fn inspector(&mut self, ui: &mut Ui) {
        let palette = self.theme.palette();
        let Some(session) = &mut self.session else {
            return;
        };
        ui.heading("Inspector");
        ui.add_space(6.0);
        let mut target = None;
        let mut offset = None;
        let mut bookmark = None;
        ScrollArea::vertical()
            .id_salt(("inspector_rows", self.version))
            .show(ui, |ui| {
                if let Some(function) = self
                    .state
                    .selected_function
                    .and_then(|key| session.analysis.functions.get(&key))
                {
                    ui.strong("Function name");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.state.name)
                            .hint_text(&function.name)
                            .desired_width(f32::INFINITY),
                    );
                    if ui
                        .add_enabled(self.opening.is_none(), Button::new("Apply name"))
                        .on_hover_text("Clear the field to restore the discovered name")
                        .clicked()
                    {
                        match session
                            .project
                            .set_function_name(function.address, self.state.name.clone())
                        {
                            Ok(()) => session.dirty = true,
                            Err(error) => self.message = Some((true, error.to_string())),
                        }
                    }
                }
                let Some(address) = self.state.selected_address else {
                    if let Some(file_offset) = self.state.selected_file_offset {
                        ui.separator();
                        ui.monospace(format!("File offset 0x{file_offset:X}"));
                        label(ui, "No mapped virtual address");
                    }
                    return;
                };
                ui.separator();
                ui.label(
                    RichText::new(format!("0x{address:X}"))
                        .monospace()
                        .color(palette.accent),
                );
                let image = session.project.image();
                if let Some(file_offset) = image.file_offset(address)
                    && ui.link(format!("File offset 0x{file_offset:X}")).clicked()
                {
                    offset = Some(file_offset);
                }
                ui.add_space(6.0);
                ui.strong("Address comment");
                ui.add(
                    egui::TextEdit::multiline(&mut self.state.note)
                        .hint_text("Add an analysis note…")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if ui
                    .add_enabled(self.opening.is_none(), Button::new("Apply comment"))
                    .clicked()
                {
                    match session
                        .project
                        .set_comment(address, self.state.note.clone())
                    {
                        Ok(()) => session.dirty = true,
                        Err(error) => self.message = Some((true, error.to_string())),
                    }
                }
                ui.strong("Bookmark");
                ui.add(
                    egui::TextEdit::singleline(&mut self.state.bookmark)
                        .hint_text("Bookmark name…")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    let active = session.project.bookmark(address).is_some();
                    if ui
                        .add_enabled(
                            !self.state.bookmark.trim().is_empty(),
                            Button::new(if active { "Rename" } else { "Add bookmark" }),
                        )
                        .clicked()
                    {
                        bookmark = Some((address, self.state.bookmark.clone()));
                    }
                    if active && ui.small_button("Remove").clicked() {
                        bookmark = Some((address, String::new()));
                    }
                });
                ui.separator();
                for (title, incoming, rows) in [
                    (
                        "Incoming references",
                        true,
                        session.references.incoming(address),
                    ),
                    (
                        "Outgoing references",
                        false,
                        session.references.outgoing(address),
                    ),
                ] {
                    ui.strong(format!("{title} ({})", rows.len()));
                    if rows.is_empty() {
                        label(ui, "None discovered");
                    }
                    ScrollArea::vertical()
                        .max_height(ROW * 10.0)
                        .id_salt(("xref", incoming, address))
                        .show_rows(ui, ROW, rows.len(), |ui, range| {
                            for reference in &rows[range] {
                                if let Some(address) = reference_row(ui, reference, incoming) {
                                    target = Some(address);
                                }
                            }
                        });
                    ui.add_space(8.0);
                }
            });
        if let Some((address, name)) = bookmark {
            self.bookmark(address, name);
        }
        if let Some(address) = target {
            self.goto(address);
        }
        if let Some(offset) = offset {
            self.show_hex(offset);
        }
    }

    fn hex(&mut self, ui: &mut Ui) {
        let palette = self.theme.palette();
        let Some(image) = self.session.as_ref().map(|session| session.project.image()) else {
            return;
        };
        ui.heading("File bytes");
        ui.label(
            RichText::new("File offset    00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F   ASCII")
                .monospace()
                .color(palette.muted),
        );
        let mut area = ScrollArea::both()
            .auto_shrink([false; 2])
            .id_salt(("hex_rows", self.version));
        if let Some(offset) = self.state.hex_jump.take() {
            area = area.vertical_scroll_offset(
                (ROW + ui.spacing().item_spacing.y) * (offset / 16).saturating_sub(3) as f32,
            );
        }
        let mut picked = None;
        let selected = self.state.selected_file_offset;
        area.show_rows(ui, ROW, image.bytes().len().div_ceil(16), |ui, rows| {
            for row in rows {
                let offset = row * 16;
                let bytes = &image.bytes()[offset..(offset + 16).min(image.bytes().len())];
                let hex = hex_bytes(bytes);
                let ascii: String = bytes
                    .iter()
                    .map(|&byte| {
                        if (b' '..=b'~').contains(&byte) {
                            char::from(byte)
                        } else {
                            '.'
                        }
                    })
                    .collect();
                let text =
                    RichText::new(format!("{offset:08X}       {hex:47}   {ascii}")).monospace();
                let highlighted = selected.is_some_and(|value| value / 16 == row);
                if ui
                    .add(
                        Button::selectable(highlighted, text)
                            .min_size(vec2(ui.available_width(), ROW)),
                    )
                    .clicked()
                {
                    picked = Some(offset);
                }
            }
        });
        if let Some(offset) = picked {
            self.select_file_offset(offset);
        }
    }

    fn results(&mut self, ui: &mut Ui, strings: bool) {
        ui.heading(if strings {
            "Printable ASCII"
        } else {
            "Find bytes"
        });
        let mut run = false;
        ui.horizontal(|ui| {
            if strings {
                ui.label("Minimum length");
                ui.add(egui::DragValue::new(&mut self.state.strings_min).range(1..=256));
            } else {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.state.query)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("48 8B ?? ??")
                        .desired_width(180.0),
                );
                run = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            }
            if ui
                .add_enabled(
                    self.state.querying.is_none(),
                    Button::new(if strings { "Scan" } else { "Find" }),
                )
                .clicked()
            {
                run = true;
            }
            if self.state.querying.is_some() {
                ui.spinner();
            }
        });
        if run {
            self.query(ui.ctx(), strings);
        }
        if let Some(session) = &self.session {
            let image = session.project.image();
            let mut offset = None;
            if strings {
                if let Some(items) = &self.state.strings {
                    offset =
                        result_rows(ui, self.version, "strings", items.len(), image, |index| {
                            let item = &items[index];
                            (
                                item.offset,
                                if item.byte_count > item.ascii.len() {
                                    format!("{}…", item.ascii)
                                } else {
                                    item.ascii.clone()
                                },
                            )
                        });
                }
            } else if let Some(hits) = &self.state.hits {
                offset = result_rows(ui, self.version, "search", hits.len(), image, |index| {
                    (hits[index], String::new())
                });
            }
            if let Some(offset) = offset {
                self.show_hex(offset);
            }
        }
    }
}

enum CallSelection {
    Function(u64),
    Site(u64, u64),
}

fn call_rows(
    ui: &mut Ui,
    version: u64,
    session: &Session,
    address: u64,
    incoming: bool,
) -> Option<CallSelection> {
    let (kind, rows): (&str, &[CallSite]) = if incoming {
        ("Callers", session.references.callers(address))
    } else {
        ("Callees", session.references.callees(address))
    };
    ui.strong(format!("{kind} ({})", rows.len()));
    ui.separator();
    if rows.is_empty() {
        label(ui, "None discovered");
    }
    let mut picked = None;
    ScrollArea::both()
        .id_salt(("call-rows", version, address, incoming))
        .show_rows(ui, ROW, rows.len(), |ui, range| {
            for call in &rows[range] {
                let peer = if incoming { call.caller } else { call.target };
                ui.horizontal(|ui| {
                    let name = session.analysis.functions.get(&peer).map_or_else(
                        || format!("0x{peer:X}"),
                        |function| session.function_name(function).to_owned(),
                    );
                    if ui
                        .link(&name)
                        .on_hover_text(format!("Open function 0x{peer:X}"))
                        .clicked()
                    {
                        picked = Some(CallSelection::Function(peer));
                    }
                    if ui
                        .small_button(format!("0x{:X}", call.site))
                        .on_hover_text("Open call site")
                        .clicked()
                    {
                        picked = Some(CallSelection::Site(call.caller, call.site));
                    }
                });
            }
        });
    picked
}

fn entropy_chart(ui: &mut Ui, triage: &ImageTriage) -> Option<usize> {
    let palette = theme::current(ui.ctx());
    let size = vec2(ui.available_width().clamp(320.0, 980.0), 142.0);
    let (response, painter) = ui.allocate_painter(size, Sense::click());
    let chart = response.rect.shrink2(vec2(0.0, 6.0));
    let graph = Rect::from_min_max(chart.min + vec2(31.0, 3.0), chart.max - vec2(8.0, 19.0));
    painter.rect_filled(chart, 3, palette.panel);
    for level in [0, 2, 4, 6, 8] {
        let y = egui::lerp(graph.bottom()..=graph.top(), level as f32 / 8.0);
        painter.line_segment(
            [pos2(graph.left(), y), pos2(graph.right(), y)],
            Stroke::new(0.7_f32, palette.border),
        );
        painter.text(
            pos2(graph.left() - 7.0, y),
            Align2::RIGHT_CENTER,
            level,
            FontId::monospace(11.0),
            palette.muted,
        );
    }
    for sample in &triage.samples {
        let start = graph.left() + graph.width() * sample.offset as f32 / triage.size_bytes as f32;
        let end = graph.left()
            + graph.width() * (sample.offset + sample.byte_count) as f32 / triage.size_bytes as f32;
        let top = egui::lerp(
            graph.bottom()..=graph.top(),
            (sample.entropy_bits_per_byte as f32 / 8.0).clamp(0.0, 1.0),
        );
        painter.rect_filled(
            Rect::from_min_max(pos2(start, top), pos2(end, graph.bottom())),
            0,
            palette.accent,
        );
    }
    painter.text(
        pos2(graph.left(), graph.bottom() + 5.0),
        Align2::LEFT_TOP,
        "0x0",
        FontId::monospace(11.0),
        palette.muted,
    );
    painter.text(
        pos2(graph.right(), graph.bottom() + 5.0),
        Align2::RIGHT_TOP,
        format!("0x{:X}", triage.size_bytes),
        FontId::monospace(11.0),
        palette.muted,
    );
    let hovered = response
        .hover_pos()
        .filter(|point| graph.contains(*point))?;
    let byte = ((hovered.x - graph.left()) / graph.width() * triage.size_bytes as f32) as usize;
    let byte = byte.min(triage.size_bytes.saturating_sub(1));
    let index = triage
        .samples
        .partition_point(|sample| sample.offset <= byte)
        .checked_sub(1)?;
    let sample = &triage.samples[index];
    let selected = response.clicked().then_some(byte);
    painter.line_segment(
        [
            pos2(hovered.x, graph.top()),
            pos2(hovered.x, graph.bottom()),
        ],
        Stroke::new(1.3_f32, palette.text),
    );
    response.on_hover_text(format!(
        "0x{:X} · {:.3} bits/byte in this window\nClick to open in Hex",
        byte, sample.entropy_bits_per_byte
    ));
    selected
}

fn graph(ui: &mut Ui, version: u64, function: &Function, selected: Option<u64>) -> Option<u64> {
    let palette = theme::current(ui.ctx());
    const LIMIT: usize = 200;
    let visible = &function.blocks[..function.blocks.len().min(LIMIT)];
    if function.blocks.len() > LIMIT {
        label(ui, "Showing the first 200 blocks by address.");
    }
    if visible.is_empty() {
        label(ui, "No blocks decoded.");
        return None;
    }
    let columns = if visible.len() == 1 { 1 } else { 2 };
    let stride = vec2(292.0, 132.0);
    let positions: BTreeMap<_, _> = visible
        .iter()
        .enumerate()
        .map(|(index, block)| {
            (
                block.address,
                vec2(
                    25.0 + (index % columns) as f32 * stride.x,
                    17.0 + (index / columns) as f32 * stride.y,
                ),
            )
        })
        .collect();
    let mut clicked = None;
    ScrollArea::both()
        .auto_shrink([false; 2])
        .id_salt(("cfg", version, function.address))
        .show(ui, |ui| {
            let size = vec2(
                stride.x * columns as f32 + 25.0,
                stride.y * visible.len().div_ceil(columns) as f32 + 26.0,
            );
            let (canvas, painter) = ui.allocate_painter(size, Sense::hover());
            let origin = canvas.rect.min;
            let block_size = vec2(256.0, 91.0);
            for block in visible {
                let base = origin + positions[&block.address];
                for edge in &block.successors {
                    let Some(&target) = positions.get(&edge.target) else {
                        continue;
                    };
                    let target = origin + target;
                    let color = match edge.kind {
                        EdgeKind::Fallthrough => palette.muted,
                        EdgeKind::Taken => palette.accent,
                        EdgeKind::Jump => palette.warn,
                    };
                    let stroke = Stroke::new(1.4_f32, color);
                    let from = base + vec2(block_size.x * 0.5, block_size.y);
                    let to = target + vec2(block_size.x * 0.5, 0.0);
                    if to.y <= from.y {
                        let x = if target.x <= base.x {
                            target.x - 12.0
                        } else {
                            target.x + block_size.x + 12.0
                        };
                        let points = vec![
                            from,
                            from + vec2(0.0, 9.0),
                            pos2(x, from.y + 9.0),
                            pos2(x, to.y - 8.0),
                            to - vec2(0.0, 8.0),
                        ];
                        painter.add(egui::Shape::line(points, stroke));
                        painter.arrow(to - vec2(0.0, 8.0), vec2(0.0, 8.0), stroke);
                    } else {
                        painter.line_segment([from, to - vec2(0.0, 8.0)], stroke);
                        painter.arrow(to - vec2(0.0, 8.0), vec2(0.0, 8.0), stroke);
                    }
                }
            }
            for block in visible {
                let rect = Rect::from_min_size(origin + positions[&block.address], block_size);
                let response =
                    ui.interact(rect, ui.id().with(("block", block.address)), Sense::click());
                if response.clicked() {
                    clicked = Some(block.address);
                }
                let highlight = response.hovered() || selected == Some(block.address);
                painter.rect(
                    rect,
                    4,
                    palette.raised,
                    Stroke::new(
                        1.0_f32,
                        if highlight {
                            palette.accent
                        } else {
                            palette.border
                        },
                    ),
                    StrokeKind::Inside,
                );
                let heading = format!(
                    "0x{:X}   ·   {} instructions",
                    block.address, block.instruction_count
                );
                painter.text(
                    rect.min + vec2(10.0, 10.0),
                    Align2::LEFT_TOP,
                    heading,
                    FontId::monospace(13.0),
                    palette.accent,
                );
                for (index, instruction) in function.instructions[block.first_instruction..]
                    .iter()
                    .take(block.instruction_count.min(3))
                    .enumerate()
                {
                    let text: String = instruction.text.chars().take(31).collect();
                    painter.text(
                        rect.min + vec2(10.0, 34.0 + index as f32 * 16.0),
                        Align2::LEFT_TOP,
                        text,
                        FontId::monospace(12.0),
                        palette.text,
                    );
                }
            }
        });
    clicked
}

fn region_row(ui: &mut Ui, region: &Region) -> bool {
    let text = format!("{}   {} B", region.name, region.size);
    ui.add_enabled(region.file_offset.is_some(), Button::new(text).truncate())
        .on_hover_text(format!(
            "{}{}",
            region
                .address
                .map(|address| format!("VA 0x{address:X}"))
                .unwrap_or_else(|| "No virtual address".into()),
            if region.executable {
                " · executable"
            } else {
                ""
            }
        ))
        .clicked()
}

fn reference_row(ui: &mut Ui, reference: &Reference, incoming: bool) -> Option<u64> {
    let address = if incoming {
        reference.source
    } else {
        reference.target
    };
    ui.link(format!("0x{address:X}  {:?}", reference.kind))
        .clicked()
        .then_some(address)
}

fn result_rows(
    ui: &mut Ui,
    version: u64,
    id: &str,
    len: usize,
    image: &Image,
    row: impl Fn(usize) -> (usize, String),
) -> Option<usize> {
    if len == 0 {
        label(ui, "No matches");
        return None;
    }
    label(
        ui,
        if len > 1000 {
            "Showing the first 1,000 matches"
        } else {
            "Select a result to open it in Hex"
        },
    );
    let mut selected = None;
    ScrollArea::both()
        .auto_shrink([false; 2])
        .id_salt((id, version))
        .show_rows(ui, ROW, len.min(1000), |ui, range| {
            Grid::new((id, "result_grid"))
                .num_columns(3)
                .spacing([12.0, 5.0])
                .show(ui, |ui| {
                    for index in range {
                        let (offset, text) = row(index);
                        if ui
                            .link(RichText::new(format!("0x{offset:08X}")).monospace())
                            .clicked()
                        {
                            selected = Some(offset);
                        }
                        ui.monospace(
                            image
                                .virtual_address(offset)
                                .map(|address| format!("0x{address:X}"))
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.label(text);
                        ui.end_row();
                    }
                });
        });
    selected
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 3);
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        let _ = write!(output, "{byte:02X}");
    }
    output
}
