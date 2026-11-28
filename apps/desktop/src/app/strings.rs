use std::collections::BTreeMap;

use eframe::egui::{self, Button, RichText, ScrollArea, SidePanel, Ui, vec2};
use hexaris_engine::{Function, Image, ReferenceKind, StringItem, string_at};

use super::{App, View, label};

pub(super) const FILE_STRING_LIMIT: usize = 1_000;
pub(super) type References = BTreeMap<u64, Vec<(u64, String)>>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Scope {
    Referenced,
    File,
}

#[derive(Default)]
pub(super) struct Drawer {
    visibility: Option<bool>,
    side_by_side: bool,
    scope: Option<Scope>,
    filter: String,
    function: Option<u64>,
    pub references: References,
}

impl Drawer {
    fn refresh(&mut self, image: &Image, function: Option<&Function>) {
        let key = function.map(|function| function.address);
        if self.function == key {
            return;
        }
        self.function = key;
        self.references.clear();
        for reference in function
            .into_iter()
            .flat_map(|function| &function.references)
        {
            if reference.kind == ReferenceKind::Address
                && let Some(text) = string_at(image, reference.target)
            {
                self.references
                    .entry(reference.source)
                    .or_default()
                    .push((reference.target, text));
            }
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.function = None;
        self.references.clear();
    }

    pub(super) fn toggle(&mut self, ui: &mut Ui, view: View) -> bool {
        let mut open = self.is_open(view);
        let changed = ui
            .toggle_value(&mut open, "Strings panel")
            .on_hover_text("Browse strings alongside this view")
            .changed();
        if changed {
            self.visibility = Some(open);
        }
        changed
    }

    fn is_open(&self, view: View) -> bool {
        self.visibility
            .unwrap_or(matches!(view, View::Flow | View::Program))
    }
}

enum Action {
    Scan,
    Code(u64),
    Bytes(usize),
}

impl App {
    pub(super) fn strings_drawer(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        let image = session.project.image();
        let function = self
            .state
            .selected_function
            .and_then(|address| session.analysis.functions.get(&address));
        let drawer = &mut self.state.string_drawer;
        drawer.refresh(image, function);
        if !drawer.is_open(self.state.view) {
            return;
        }
        let side_by_side = ui.available_width() >= 620.0;
        if drawer.side_by_side != side_by_side {
            drawer.side_by_side = side_by_side;
            self.state.flow.refit = true;
        }
        let items = self.state.strings.as_deref();
        let scanning = self.state.querying.is_some();
        let action = if side_by_side {
            SidePanel::right(egui::Id::new(("workspace-strings", self.version)))
                .resizable(true)
                .default_width(244.0)
                .width_range(190.0..=320.0)
                .show_inside(ui, |ui| panel(ui, drawer, image, items, scanning))
                .inner
        } else {
            let action = panel(ui, drawer, image, items, scanning);
            ui.separator();
            action
        };
        match action {
            Some(Action::Scan) => self.query(ui.ctx(), true),
            Some(Action::Bytes(offset)) => self.show_hex(offset),
            Some(Action::Code(address)) => {
                if let Some(function) = self.state.selected_function {
                    self.navigate(function, address);
                    self.state.flow.focus_selection = true;
                    self.state.program_jump = Some(address);
                    if self.state.view == View::Pseudocode {
                        self.state.pseudo_control_flow = true;
                        self.state.pseudo_jump = self
                            .session
                            .as_ref()
                            .and_then(|session| session.pseudocode.get(&function))
                            .and_then(|result| result.control_flow.as_ref())
                            .and_then(|listing| {
                                listing
                                    .lines
                                    .iter()
                                    .position(|line| line.address == Some(address))
                            });
                    }
                }
            }
            None => {}
        }
    }
}

fn panel(
    ui: &mut Ui,
    drawer: &mut Drawer,
    image: &Image,
    items: Option<&[StringItem]>,
    scanning: bool,
) -> Option<Action> {
    let count = drawer.references.values().map(Vec::len).sum::<usize>();
    let automatic = if count == 0 {
        Scope::File
    } else {
        Scope::Referenced
    };
    let current = drawer.scope.unwrap_or(automatic);
    let mut scope = current;
    ui.strong("Strings");
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(
            &mut scope,
            Scope::Referenced,
            format!("Referenced ({count})"),
        );
        ui.selectable_value(&mut scope, Scope::File, "File strings");
    });
    if scope != current {
        drawer.scope = Some(scope);
    }
    ui.add(
        egui::TextEdit::singleline(&mut drawer.filter)
            .desired_width(150.0)
            .char_limit(100)
            .hint_text("Filter listed strings…"),
    );
    if scope == Scope::File {
        label(
            ui,
            if drawer.function.is_some() && count == 0 {
                "File scan · No direct string references in this function"
            } else {
                "File scan · Printable ASCII"
            },
        );
    }
    let file_items = items.unwrap_or_default();
    if scope == Scope::File && file_items.len() > FILE_STRING_LIMIT {
        label(
            ui,
            "First 1,000 strings. Ctrl+F → Binary strings searches the full file.",
        );
    }
    let filter = drawer.filter.trim().to_ascii_lowercase();
    let matches = |text: &str| filter.is_empty() || text.to_ascii_lowercase().contains(&filter);
    let rows: Vec<_> = match scope {
        Scope::Referenced => drawer
            .references
            .iter()
            .flat_map(|(&source, strings)| {
                strings.iter().filter(|(_, text)| matches(text)).filter_map(
                    move |(target, text)| {
                        Some((Some(source), image.file_offset(*target)?, text.as_str()))
                    },
                )
            })
            .collect(),
        Scope::File => file_items
            .iter()
            .take(FILE_STRING_LIMIT)
            .filter(|item| matches(&item.ascii))
            .map(|item| (None, item.offset, item.ascii.as_str()))
            .collect(),
    };
    let height = if drawer.side_by_side {
        ui.available_height().max(0.0)
    } else {
        82.0
    };
    if rows.is_empty() {
        ui.allocate_ui(vec2(ui.available_width(), height), |ui| {
            label(
                ui,
                if scope == Scope::File && items.is_none() {
                    "Scanning file strings…"
                } else if scope == Scope::Referenced && count == 0 {
                    "No direct string references."
                } else if !filter.is_empty() {
                    "No matching strings."
                } else {
                    "No strings found."
                },
            );
        });
        return (scope == Scope::File && items.is_none() && !scanning).then_some(Action::Scan);
    }
    let mut action = None;
    let row_height = ui.spacing().interact_size.y;
    ScrollArea::vertical()
        .id_salt((
            "strings-rows",
            (scope == Scope::Referenced).then_some(drawer.function),
        ))
        .max_height(height)
        .auto_shrink([false; 2])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for &(source, offset, text) in &rows[range] {
                ui.horizontal(|ui| {
                    ui.set_min_height(row_height);
                    if let Some(source) = source {
                        if ui
                            .link(format!("0x{source:X}"))
                            .on_hover_text("Select the referencing instruction")
                            .clicked()
                        {
                            action = Some(Action::Code(source));
                        }
                    } else {
                        ui.monospace(format!("+0x{offset:X}"));
                    }
                    let preview = format!("{text:?}");
                    let row = ui
                        .add(
                            Button::new(RichText::new(&preview).monospace())
                                .small()
                                .truncate()
                                .frame(false),
                        )
                        .on_hover_text(format!("{preview}\nClick to open in Hex"));
                    if row.clicked() {
                        action = Some(Action::Bytes(offset));
                    }
                    row.context_menu(|ui| {
                        if ui.button("Copy string").clicked() {
                            ui.ctx().copy_text(text.to_owned());
                            ui.close();
                        }
                    });
                });
            }
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexaris_engine::{Architecture, LoadMode, analyze_function, scan_strings};

    #[test]
    fn missing_references_trigger_the_file_scan_and_show_its_results() {
        let image = Image::from_bytes(
            b"\xc3\0Fixture label\0".to_vec(),
            LoadMode::Raw {
                architecture: Architecture::X86_64,
                base: 0x1000,
            },
        )
        .unwrap();
        let function = analyze_function(&image, 0x1000).unwrap();
        let mut drawer = Drawer::default();
        drawer.refresh(&image, Some(&function));
        assert!(drawer.references.is_empty());
        let ctx = egui::Context::default();
        let mut action = None;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                action = panel(ui, &mut drawer, &image, None, false);
            });
        });
        assert!(matches!(action, Some(Action::Scan)));
        let file_strings: Vec<_> = scan_strings(image.bytes(), 5).collect();
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                action = panel(ui, &mut drawer, &image, Some(&file_strings), false);
            });
        });
        assert!(action.is_none());
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("Fixture label"))
        }));
    }
}
