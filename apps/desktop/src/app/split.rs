use std::mem;

use eframe::egui::{self, SidePanel, TopBottomPanel, Ui};
use hexaris_engine::LinearListing;

use super::{App, View, ViewState, flow};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Layout {
    #[default]
    Automatic,
    SideBySide,
    Stacked,
}

#[derive(Default)]
pub(super) struct Split {
    enabled: bool,
    layout: Layout,
    secondary: Option<Pane>,
    last_selection: (Option<u64>, Option<u64>),
}

#[derive(Default)]
struct Pane {
    view: View,
    preferred_code_view: View,
    flow: flow::GraphState,
    linear: Option<LinearListing>,
    hex_jump: Option<usize>,
    hex_offset: String,
    disassembly_jump: Option<usize>,
    pseudo_jump: Option<usize>,
    pseudo_control_flow: bool,
    program_jump: Option<u64>,
    program_region: Option<(u64, u64)>,
    hide_program_names: bool,
    hide_program_gaps: bool,
    reveal: bool,
}

impl Pane {
    fn swap(&mut self, state: &mut ViewState) {
        mem::swap(&mut self.view, &mut state.view);
        mem::swap(
            &mut self.preferred_code_view,
            &mut state.preferred_code_view,
        );
        mem::swap(&mut self.flow, &mut state.flow);
        mem::swap(&mut self.linear, &mut state.linear);
        mem::swap(&mut self.hex_jump, &mut state.hex_jump);
        mem::swap(&mut self.hex_offset, &mut state.hex_offset);
        mem::swap(&mut self.disassembly_jump, &mut state.disassembly_jump);
        mem::swap(&mut self.pseudo_jump, &mut state.pseudo_jump);
        mem::swap(
            &mut self.pseudo_control_flow,
            &mut state.pseudo_control_flow,
        );
        mem::swap(&mut self.program_jump, &mut state.program_jump);
        mem::swap(&mut self.program_region, &mut state.program_region);
        mem::swap(&mut self.hide_program_names, &mut state.hide_program_names);
        mem::swap(&mut self.hide_program_gaps, &mut state.hide_program_gaps);
    }
}

impl Split {
    pub(super) fn controls(&mut self, ui: &mut Ui, view: View) -> bool {
        let changed = ui.toggle_value(&mut self.enabled, "Split view").changed();
        if self.enabled {
            self.secondary.get_or_insert_with(|| {
                let other = if view == View::Pseudocode {
                    View::Disassembly
                } else {
                    View::Pseudocode
                };
                Pane {
                    view: other,
                    preferred_code_view: other,
                    reveal: true,
                    ..Pane::default()
                }
            });
            let before = self.layout;
            ui.menu_button("Split layout", |ui| {
                for (layout, name) in [
                    (Layout::Automatic, "Automatic"),
                    (Layout::SideBySide, "Side by side"),
                    (Layout::Stacked, "Stacked"),
                ] {
                    if ui
                        .selectable_value(&mut self.layout, layout, name)
                        .clicked()
                    {
                        ui.close();
                    }
                }
            });
            let resized = changed || self.layout != before;
            if resized && let Some(secondary) = &mut self.secondary {
                secondary.flow.refit = true;
                secondary.reveal = true;
            }
            resized
        } else {
            changed
        }
    }

    pub(super) fn refit(&mut self) {
        if let Some(secondary) = &mut self.secondary {
            secondary.flow.refit = true;
        }
    }
}

impl App {
    pub(super) fn render_workspace_panes(&mut self, ui: &mut Ui) {
        if !self.state.split.enabled {
            self.content_view(ui);
            return;
        }
        let Some(mut secondary) = self.state.split.secondary.take() else {
            self.content_view(ui);
            return;
        };
        let size = ui.available_size();
        let stacked = match self.state.split.layout {
            Layout::Automatic => size.x < 780.0 && size.y >= 250.0,
            Layout::SideBySide => false,
            Layout::Stacked => true,
        };
        let before = (self.state.selected_function, self.state.selected_address);
        if before != self.state.split.last_selection {
            secondary.reveal = true;
        }
        let version = self.version;
        let mut other = |ui: &mut Ui| {
            ui.horizontal_wrapped(|ui| {
                let previous = secondary.view;
                egui::ComboBox::from_id_salt("secondary-view")
                    .selected_text(secondary.view.label())
                    .show_ui(ui, |ui| {
                        for view in View::ALL {
                            ui.selectable_value(&mut secondary.view, view, view.label());
                        }
                    });
                if secondary.view != previous {
                    if secondary.view.is_code() {
                        secondary.preferred_code_view = secondary.view;
                    }
                    secondary.reveal = true;
                }
                if ui.small_button("Close pane").clicked() {
                    self.state.split.enabled = false;
                    self.state.flow.refit = true;
                }
            });
            ui.separator();
            secondary.swap(&mut self.state);
            let keep = mem::take(&mut self.state.reset_code_view);
            if mem::take(&mut secondary.reveal) {
                self.reveal_selection();
            }
            self.content_view(ui);
            self.state.reset_code_view = keep;
            secondary.swap(&mut self.state);
        };
        if stacked {
            let height = size.y.max(80.0);
            TopBottomPanel::bottom(egui::Id::new(("split-secondary-bottom", version)))
                .resizable(true)
                .default_height(height * 0.5)
                .height_range(height * 0.3..=height * 0.7)
                .show_inside(ui, &mut other);
        } else {
            let width = size.x.max(140.0);
            SidePanel::right(egui::Id::new(("split-secondary-right", version)))
                .resizable(true)
                .default_width(width * 0.5)
                .width_range(width * 0.3..=width * 0.7)
                .show_inside(ui, &mut other);
        }
        let after_other = (self.state.selected_function, self.state.selected_address);
        if after_other != before {
            self.reveal_selection();
        }
        self.content_view(ui);
        if (self.state.selected_function, self.state.selected_address) != after_other {
            secondary.reveal = true;
            ui.ctx().request_repaint();
        }
        self.state.split.last_selection =
            (self.state.selected_function, self.state.selected_address);
        self.state.split.secondary = Some(secondary);
    }

    fn reveal_selection(&mut self) {
        let Some(address) = self.state.selected_address else {
            return;
        };
        match self.state.view {
            View::Flow => self.state.flow.focus_selection = true,
            View::Program => self.state.program_jump = Some(address),
            View::Linear => {
                if let Some(session) = &self.session
                    && let Ok(listing) =
                        hexaris_engine::disassemble(session.project.image(), address, 512)
                {
                    self.state.linear = Some(listing);
                }
            }
            View::Hex => self.state.hex_jump = self.state.selected_file_offset,
            View::Disassembly => {
                self.state.disassembly_jump = self.session.as_ref().and_then(|session| {
                    let function = session
                        .analysis
                        .functions
                        .get(&self.state.selected_function?)?;
                    function
                        .instructions
                        .binary_search_by_key(&address, |instruction| instruction.address)
                        .ok()
                });
            }
            View::Pseudocode => {
                self.state.pseudo_jump = self
                    .session
                    .as_ref()
                    .and_then(|session| session.pseudocode.get(&self.state.selected_function?))
                    .filter(|result| self.state.pseudo_control_flow || result.code.is_none())
                    .and_then(|result| result.control_flow.as_ref())
                    .and_then(|listing| {
                        listing
                            .lines
                            .iter()
                            .position(|line| line.address == Some(address))
                    });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_swap_preserves_private_views_and_jump_requests() {
        let mut state = ViewState {
            view: View::Program,
            program_jump: Some(0x1000),
            selected_address: Some(0x2000),
            ..Default::default()
        };
        let mut second = Pane {
            view: View::Pseudocode,
            pseudo_jump: Some(4),
            ..Default::default()
        };
        second.swap(&mut state);
        assert_eq!(state.view, View::Pseudocode);
        assert_eq!(state.pseudo_jump, Some(4));
        assert_eq!(state.selected_address, Some(0x2000));
        state.pseudo_jump = Some(7);
        second.swap(&mut state);
        assert_eq!(state.view, View::Program);
        assert_eq!(state.program_jump, Some(0x1000));
        assert_eq!(second.pseudo_jump, Some(7));
    }
}
