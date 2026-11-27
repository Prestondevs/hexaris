use std::{collections::BTreeMap, fmt::Write as _};

use eframe::egui::{self, Align2, FontId, Grid, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use hexaris_engine::{EdgeKind, Image, Reference, Region};

use super::*;
use crate::theme::{BORDER, RAISED, TEXT};

const ROW: f32 = 22.0;

impl App {
    pub(super) fn navigator(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        ui.heading("Image browser");
        let info = &session.project.image().info;
        ui.label(
            RichText::new(format!("{}  /  {}", info.format, info.architecture.name()))
                .color(ACCENT),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.browser_tab, 0, "Functions");
            ui.selectable_value(&mut self.browser_tab, 1, "Regions");
            ui.selectable_value(&mut self.browser_tab, 2, "Symbols");
            ui.selectable_value(&mut self.browser_tab, 3, "Imports");
            ui.selectable_value(&mut self.browser_tab, 4, "Exports");
        });
        ui.separator();
        let mut target = None;
        let mut offset = None;
        match self.browser_tab {
            0 => {
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text("Filter functions…")
                        .desired_width(f32::INFINITY),
                );
                label(
                    ui,
                    &format!("{} discovered", session.analysis.functions.len()),
                );
                let filter = self.filter.to_lowercase();
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
                                self.selected_function == Some(function.address),
                                RichText::new(text).monospace(),
                            )
                            .truncate()
                            .min_size(vec2(ui.available_width(), ROW));
                            if ui.add(button).clicked() {
                                target = Some(function.address);
                            }
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
            _ => {
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
        }
        if let Some(address) = target {
            self.goto(address);
        }
        if let Some(offset) = offset {
            self.show_hex(offset);
        }
    }

    pub(super) fn workspace(&mut self, ui: &mut Ui) {
        let mut nav = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.navigation_back.is_empty(), Button::new("Back"))
                .on_hover_text("Back")
                .clicked()
            {
                self.history(false);
            }
            if ui
                .add_enabled(!self.navigation_forward.is_empty(), Button::new("Forward"))
                .on_hover_text("Forward")
                .clicked()
            {
                self.history(true);
            }
            let address_input = ui.add(
                egui::TextEdit::singleline(&mut self.jump_text)
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
            match parse_address(&self.jump_text) {
                Ok(address) if analyze => self.analyze_at(address),
                Ok(address) if self.view == View::Linear => self.decode_linear(address),
                Ok(address) => self.goto(address),
                Err(_) => {
                    self.error("Enter a decimal address or prefix a hexadecimal address with 0x.")
                }
            }
        }
        ui.add_space(5.0);
        let previous = self.view;
        ui.horizontal_wrapped(|ui| {
            for view in View::ALL {
                ui.selectable_value(&mut self.view, view, view.label());
            }
        });
        if self.view == View::Linear
            && previous != View::Linear
            && let Some(address) = self.selected_address
        {
            self.decode_linear(address);
        }
        ui.separator();
        if let Some(session) = &self.session {
            for notice in &session.analysis.notices {
                ui.label(RichText::new(notice).color(WARN));
            }
            for warning in &session.project.image().info.metadata_warnings {
                ui.label(RichText::new(warning).color(WARN));
            }
        }
        match self.view {
            View::Disassembly | View::Flow | View::Registers | View::Pseudocode => {
                self.function_view(ui)
            }
            View::Linear => self.linear_view(ui),
            View::Hex => self.hex(ui),
            View::Strings => self.results(ui, true),
            View::Search => self.results(ui, false),
        }
    }

    fn linear_view(&mut self, ui: &mut Ui) {
        let Some(listing) = &self.linear else {
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
                .color(WARN),
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
                                self.selected_address == Some(instruction.address),
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
            self.select(self.selected_function, Some(address));
        }
        if let Some(address) = next {
            self.decode_linear(address);
        }
    }

    fn function_view(&mut self, ui: &mut Ui) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(function) = self
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
                    .color(ACCENT),
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
                    ui.label(RichText::new(issue).color(WARN));
                }
            });
        }
        let mut target = None;
        match self.view {
            View::Disassembly => {
                let mut area = ScrollArea::both()
                    .id_salt(("disassembly", self.version, function.address))
                    .auto_shrink([false; 2]);
                if let Some(row) = self.disassembly_jump.take() {
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
                            self.selected_address == Some(instruction.address),
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
                if let Some(address) = graph(ui, self.version, function, self.selected_address) {
                    target = Some(address);
                    self.view = View::Disassembly;
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
                                    .color(ACCENT)
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
                                .color(if statement.supported { TEXT } else { MUTED });
                                if ui
                                    .selectable_label(
                                        self.selected_address == Some(statement.address),
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
                    ui.label(RichText::new(reason).color(WARN));
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
        let Some(session) = &mut self.session else {
            return;
        };
        ui.heading("Inspector");
        ui.add_space(6.0);
        let mut target = None;
        let mut offset = None;
        ScrollArea::vertical()
            .id_salt(("inspector_rows", self.version))
            .show(ui, |ui| {
                if let Some(function) = self
                    .selected_function
                    .and_then(|key| session.analysis.functions.get(&key))
                {
                    ui.strong("Function name");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name)
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
                            .set_function_name(function.address, self.name.clone())
                        {
                            Ok(()) => session.dirty = true,
                            Err(error) => self.message = Some((true, error.to_string())),
                        }
                    }
                }
                let Some(address) = self.selected_address else {
                    return;
                };
                ui.separator();
                ui.label(
                    RichText::new(format!("0x{address:X}"))
                        .monospace()
                        .color(ACCENT),
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
                    egui::TextEdit::multiline(&mut self.note)
                        .hint_text("Add an analysis note…")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if ui
                    .add_enabled(self.opening.is_none(), Button::new("Apply comment"))
                    .clicked()
                {
                    match session.project.set_comment(address, self.note.clone()) {
                        Ok(()) => session.dirty = true,
                        Err(error) => self.message = Some((true, error.to_string())),
                    }
                }
                ui.separator();
                let references = || {
                    session
                        .analysis
                        .functions
                        .values()
                        .flat_map(|function| &function.references)
                };
                for (title, incoming) in [
                    ("Incoming references", true),
                    ("Outgoing references", false),
                ] {
                    ui.strong(title);
                    let mut rows = references()
                        .filter(|reference| {
                            if incoming {
                                reference.target == address
                            } else {
                                reference.source == address
                            }
                        })
                        .peekable();
                    if rows.peek().is_none() {
                        label(ui, "None discovered");
                    }
                    for reference in rows.take(80) {
                        if let Some(address) = reference_row(ui, reference, incoming) {
                            target = Some(address);
                        }
                    }
                    ui.add_space(8.0);
                }
            });
        if let Some(address) = target {
            self.goto(address);
        }
        if let Some(offset) = offset {
            self.show_hex(offset);
        }
    }

    fn hex(&mut self, ui: &mut Ui) {
        let Some(image) = self.session.as_ref().map(|session| session.project.image()) else {
            return;
        };
        ui.heading("File bytes");
        ui.label(
            RichText::new("File offset    00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F   ASCII")
                .monospace()
                .color(MUTED),
        );
        let mut area = ScrollArea::both()
            .auto_shrink([false; 2])
            .id_salt(("hex_rows", self.version));
        if let Some(offset) = self.hex_jump.take() {
            area = area.vertical_scroll_offset(
                (ROW + ui.spacing().item_spacing.y) * (offset / 16).saturating_sub(3) as f32,
            );
        }
        let mut picked = None;
        let selected = self
            .selected_address
            .and_then(|address| image.file_offset(address));
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
            self.select(self.selected_function, image.virtual_address(offset));
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
                ui.add(egui::DragValue::new(&mut self.strings_min).range(1..=256));
            } else {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("48 8B ?? ??")
                        .desired_width(180.0),
                );
                run = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            }
            if ui
                .add_enabled(
                    self.querying.is_none(),
                    Button::new(if strings { "Scan" } else { "Find" }),
                )
                .clicked()
            {
                run = true;
            }
            if self.querying.is_some() {
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
                if let Some(items) = &self.strings {
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
            } else if let Some(hits) = &self.hits {
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

fn graph(ui: &mut Ui, version: u64, function: &Function, selected: Option<u64>) -> Option<u64> {
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
                        EdgeKind::Fallthrough => MUTED,
                        EdgeKind::Taken => ACCENT,
                        EdgeKind::Jump => WARN,
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
                    RAISED,
                    Stroke::new(1.0_f32, if highlight { ACCENT } else { BORDER }),
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
                    ACCENT,
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
                        TEXT,
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
