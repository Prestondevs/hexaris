use std::collections::BTreeMap;

use eframe::egui::{self, Button, ScrollArea, Sense, Ui, vec2};
use hexaris_engine::{Analysis, BytePatch, Function, Image, Instruction};

use super::{
    App, label,
    views::{instruction_text, patch_options},
};

pub(super) struct Row {
    pub address: u64,
    function: u64,
    instruction: usize,
}

#[derive(Clone, Copy)]
enum LineKind {
    Code(usize),
    Gap(u64),
}

#[derive(Clone, Copy)]
struct Line {
    address: u64,
    kind: LineKind,
}

pub(super) struct Index {
    pub instructions: Vec<Row>,
    mapped: Vec<Line>,
}

impl Index {
    fn line(&self, index: usize, include_gaps: bool) -> Line {
        if include_gaps {
            self.mapped[index]
        } else {
            Line {
                address: self.instructions[index].address,
                kind: LineKind::Code(index),
            }
        }
    }

    fn boundary(&self, address: u64, include_gaps: bool) -> usize {
        if include_gaps {
            self.mapped.partition_point(|line| line.address < address)
        } else {
            self.instructions
                .partition_point(|row| row.address < address)
        }
    }

    fn position(&self, address: u64, include_gaps: bool) -> usize {
        let index = self.boundary(address, include_gaps);
        if include_gaps
            && index > 0
            && matches!(self.mapped[index - 1].kind, LineKind::Gap(end) if end > address)
        {
            index - 1
        } else {
            index
        }
    }

    pub fn contains_gap(&self, address: u64) -> bool {
        let row = self.position(address, true);
        self.mapped.get(row).is_some_and(|line| {
            line.address <= address && matches!(line.kind, LineKind::Gap(end) if end > address)
        })
    }
}

impl Row {
    pub fn resolve<'a>(&self, analysis: &'a Analysis) -> (&'a Function, &'a Instruction) {
        let function = &analysis.functions[&self.function];
        (function, &function.instructions[self.instruction])
    }
}

pub(super) fn index(analysis: &Analysis, image: &Image) -> Index {
    let mut rows = BTreeMap::new();
    for function in analysis.functions.values() {
        for (index, instruction) in function.instructions.iter().enumerate() {
            let row = Row {
                address: instruction.address,
                function: function.address,
                instruction: index,
            };
            if row.address == row.function {
                rows.insert(row.address, row);
            } else {
                rows.entry(row.address).or_insert(row);
            }
        }
    }
    let instructions: Vec<_> = rows.into_values().collect();
    let mut ranges: Vec<_> = image
        .info
        .sections
        .iter()
        .chain(&image.info.segments)
        .filter(|region| region.executable)
        .filter_map(bounds)
        .collect();
    ranges.sort_unstable();
    let mut merged: Vec<(u64, u64)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    let mut gaps = Vec::new();
    for (start, end) in merged {
        let first = instructions.partition_point(|row| row.address < start);
        let last = instructions.partition_point(|row| row.address < end);
        let mut cursor = start;
        for row in &instructions[first..last] {
            if cursor < row.address {
                gaps.push(Line {
                    address: cursor,
                    kind: LineKind::Gap(row.address),
                });
            }
            cursor = cursor.max(
                row.address
                    .saturating_add(row.resolve(analysis).1.bytes.len() as u64),
            );
        }
        if cursor < end {
            gaps.push(Line {
                address: cursor,
                kind: LineKind::Gap(end),
            });
        }
    }
    let mut mapped: Vec<_> = instructions
        .iter()
        .enumerate()
        .map(|(index, row)| Line {
            address: row.address,
            kind: LineKind::Code(index),
        })
        .collect();
    mapped.extend(gaps);
    mapped.sort_unstable_by_key(|line| line.address);
    Index {
        instructions,
        mapped,
    }
}

impl App {
    pub(super) fn program_view(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        let image = session.project.image();
        let palette = self.theme.palette();
        let mut jump = self.state.program_jump.take();
        if let Some(address) = jump
            && self
                .state
                .program_region
                .is_some_and(|(start, end)| !(start..end).contains(&address))
        {
            self.state.program_region = None;
        }
        ui.horizontal_wrapped(|ui| {
            ui.heading("Program listing");
            let scope_name = image
                .info
                .sections
                .iter()
                .chain(&image.info.segments)
                .find(|region| region.executable && bounds(region) == self.state.program_region)
                .map_or("All executable regions", |region| region.name.as_str());
            egui::ComboBox::from_id_salt(("program-region", self.version))
                .selected_text(scope_name)
                .show_ui(ui, |ui| {
                    let old = self.state.program_region;
                    ui.selectable_value(
                        &mut self.state.program_region,
                        None,
                        "All executable regions",
                    );
                    for region in image
                        .info
                        .sections
                        .iter()
                        .chain(&image.info.segments)
                        .filter(|r| r.executable)
                    {
                        if let Some(range) = bounds(region) {
                            ui.selectable_value(
                                &mut self.state.program_region,
                                Some(range),
                                &region.name,
                            );
                        }
                    }
                    if self.state.program_region != old {
                        jump = Some(self.state.program_region.map_or(0, |(start, _)| start));
                    }
                });
            let mut names = !self.state.hide_program_names;
            if ui.toggle_value(&mut names, "Function labels").changed() {
                self.state.hide_program_names = !names;
            }
            let mut gaps = !self.state.hide_program_gaps;
            if ui.toggle_value(&mut gaps, "Undecoded ranges").changed() {
                self.state.hide_program_gaps = !gaps;
                jump = self.state.selected_address;
            }
        });
        let program = &session.program;
        let include_gaps = !self.state.hide_program_gaps;
        let length = if include_gaps {
            program.mapped.len()
        } else {
            program.instructions.len()
        };
        let scope = self.state.program_region.map_or(0..length, |(start, end)| {
            program.position(start, include_gaps)..program.boundary(end, include_gaps)
        });
        let instruction_count = self
            .state
            .program_region
            .map_or(program.instructions.len(), |(start, end)| {
                program.boundary(end, false) - program.boundary(start, false)
            });
        ui.horizontal_wrapped(|ui| {
            label(
                ui,
                &format!(
                    "{} decoded instructions · {} discovered functions in the file",
                    instruction_count,
                    session.analysis.functions.len()
                ),
            );
            if !scope.is_empty() {
                if ui.small_button("Start").clicked() {
                    jump = Some(program.line(scope.start, include_gaps).address);
                }
                if ui.small_button("End").clicked() {
                    jump = Some(program.line(scope.end - 1, include_gaps).address);
                }
            }
        });
        if scope.is_empty() {
            label(
                ui,
                "No decoded code here. Enter an executable address and choose Analyze here or open Linear.",
            );
            return;
        }
        let row_height = 22.0;
        let mut area = ScrollArea::both()
            .id_salt(("program-rows", self.version))
            .auto_shrink([false; 2]);
        if let Some(address) = jump {
            let index = program
                .position(address, include_gaps)
                .saturating_sub(scope.start);
            area = area.vertical_scroll_offset(
                (row_height + ui.spacing().item_spacing.y) * index.saturating_sub(2) as f32,
            );
        }
        let mut target = None;
        let mut follow = None;
        let mut patch_offset = None;
        let mut branch_patch: Option<(BytePatch, String)> = None;
        let mut decode = None;
        area.show_rows(ui, row_height, scope.len(), |ui, range| {
            ui.spacing_mut().interact_size.y = row_height;
            ui.spacing_mut().button_padding.y = 2.0;
            for index in range {
                let line = program.line(scope.start + index, include_gaps);
                let entry = match line.kind {
                    LineKind::Code(index) => &program.instructions[index],
                    LineKind::Gap(end) => {
                        let here = self
                            .state
                            .selected_address
                            .filter(|a| (line.address..end).contains(a));
                        let start = line.address;
                        let gap = egui::RichText::new(format!(
                            "{start:016X}   {:>7} bytes   Undecoded executable range",
                            end - start
                        ))
                        .monospace()
                        .color(palette.muted);
                        if ui
                            .add(
                                Button::selectable(here.is_some(), gap)
                                    .wrap_mode(egui::TextWrapMode::Extend)
                                    .min_size(vec2(ui.available_width(), row_height)),
                            )
                            .on_hover_text("Click to inspect this range in Linear")
                            .clicked()
                        {
                            decode = Some(here.unwrap_or(start));
                        }
                        continue;
                    }
                };
                let (function, instruction) = entry.resolve(&session.analysis);
                let comment = session
                    .project
                    .comment(entry.address)
                    .and_then(|comment| comment.lines().next())
                    .unwrap_or_default();
                let caption = if self.state.hide_program_names || function.address != entry.address
                {
                    comment.to_owned()
                } else if comment.is_empty() {
                    session.function_name(function).to_owned()
                } else {
                    format!("{} · {comment}", session.function_name(function))
                };
                let text = instruction_text(instruction, &caption, palette, self.state.number_base);
                let selected = self.state.selected_address == Some(entry.address)
                    || self.state.selection.contains(entry.address);
                let response = ui.add(
                    Button::selectable(selected, text)
                        .sense(Sense::click_and_drag())
                        .wrap_mode(egui::TextWrapMode::Extend)
                        .min_size(vec2(ui.available_width(), row_height)),
                );
                self.state.selection.interact(
                    ui,
                    &response,
                    entry.address,
                    self.state.selected_address,
                );
                if response.clicked() {
                    target = Some((entry.function, entry.address));
                }
                response.context_menu(|ui| {
                    for reference in session.references.outgoing(entry.address).iter().take(8) {
                        if ui
                            .button(format!("Follow 0x{:X}…", reference.target))
                            .clicked()
                        {
                            follow = Some(reference.target);
                            ui.close();
                        }
                    }
                    patch_options(ui, instruction, &mut patch_offset, &mut branch_patch);
                    if ui.button("Copy instruction").clicked() {
                        ui.ctx().copy_text(instruction.text.clone());
                        ui.close();
                    }
                });
            }
        });
        if let Some((function, address)) = target {
            self.navigate(function, address);
        }
        if let Some(address) = follow {
            self.goto(address);
        }
        if let Some(address) = decode {
            self.decode_linear(address);
        }
        if let Some((patch, description)) = branch_patch {
            self.open_branch_patch(patch, description);
        } else if let Some(offset) = patch_offset {
            self.open_patch(Some(offset));
        }
    }
}

fn bounds(region: &hexaris_engine::Region) -> Option<(u64, u64)> {
    let start = region.address?;
    let size = region.size.min(region.file_size);
    let end = start.checked_add(size)?;
    (end > start).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexaris_engine::{Architecture, LoadMode, analyze_function};

    #[test]
    fn program_interleaves_discovered_functions_with_mapped_gaps() {
        let base = 0x1000;
        let image = Image::from_bytes(
            vec![0xc3, 0xcc, 0xcc, 0xc3],
            LoadMode::Raw {
                architecture: Architecture::X86_64,
                base,
            },
        )
        .unwrap();
        let analysis = Analysis {
            functions: [base, base + 3]
                .into_iter()
                .map(|address| (address, analyze_function(&image, address).unwrap()))
                .collect(),
            notices: Vec::new(),
        };
        let program = index(&analysis, &image);
        assert_eq!(
            program
                .instructions
                .iter()
                .map(|row| row.address)
                .collect::<Vec<_>>(),
            [base, base + 3]
        );
        assert_eq!(program.mapped.len(), 3);
        assert!(matches!(program.mapped[1].kind, LineKind::Gap(end) if end == base + 3));
        assert!(program.contains_gap(base + 1));
        assert!(program.contains_gap(base + 2));
        assert_eq!(program.position(base + 2, true), 1);
        assert!(!program.contains_gap(base + 3));
    }
}
