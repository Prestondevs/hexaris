use std::collections::BTreeSet;

use hexaris_engine::{
    AnnotationProposal, AssistantResponse, BuildComparison, ChangeKind, ContextAddress,
    FunctionContext, MAX_RESPONSE_BYTES, ProposalKind, compare_builds,
};

use super::{App, Button, Context, RichText, ScrollArea, Session, Ui, egui};

const CONTEXT_PAGE: usize = 128;

pub(super) enum ToolsDialog {
    Context(ContextDialog),
    Review(ReviewDialog),
    Compare(ComparisonDialog),
}

pub(super) struct ContextDialog {
    tab: u64,
    context: FunctionContext,
    markdown: String,
    json: String,
    preview_json: bool,
}

#[derive(Default)]
pub(super) struct ReviewDialog {
    tab: u64,
    input: String,
    response: Option<AssistantResponse>,
    applied: BTreeSet<usize>,
    error: Option<String>,
}

pub(super) struct ComparisonDialog {
    before: u64,
    after: u64,
    report: Option<BuildComparison>,
    show_unchanged: bool,
    error: Option<String>,
}

impl App {
    pub(super) fn tools_menu(&mut self, ui: &mut Ui) {
        ui.menu_button("Tools", |ui| {
            if ui
                .add_enabled(
                    self.state.selected_function.is_some(),
                    Button::new("Function context…"),
                )
                .clicked()
                && let Some(address) = self.state.selected_function
            {
                self.open_context(address);
                ui.close();
            }
            if ui
                .add_enabled(
                    self.session.is_some(),
                    Button::new("Review assistant response…"),
                )
                .clicked()
            {
                self.open_review();
                ui.close();
            }
            if ui
                .add_enabled(
                    self.session.is_some() && !self.background.is_empty(),
                    Button::new("Compare builds…"),
                )
                .clicked()
            {
                self.open_comparison();
                ui.close();
            }
        });
    }

    pub(super) fn open_context(&mut self, address: u64) {
        match self.make_context(self.version, address, 0) {
            Ok(dialog) => self.tools_dialog = Some(ToolsDialog::Context(dialog)),
            Err(error) => self.error(error),
        }
    }

    pub(super) fn open_review(&mut self) {
        self.tools_dialog = Some(ToolsDialog::Review(ReviewDialog {
            tab: self.version,
            ..Default::default()
        }));
    }

    pub(super) fn open_comparison(&mut self) {
        if let Some((&before, _)) = self.background.last_key_value() {
            self.tools_dialog = Some(ToolsDialog::Compare(ComparisonDialog {
                before,
                after: self.version,
                report: None,
                show_unchanged: false,
                error: None,
            }));
        }
    }

    fn session_for(&self, tab: u64) -> Option<&Session> {
        if tab == self.version {
            self.session.as_ref()
        } else {
            self.background.get(&tab).map(|tab| &tab.session)
        }
    }

    fn make_context(
        &self,
        tab: u64,
        address: u64,
        offset: usize,
    ) -> hexaris_engine::Result<ContextDialog> {
        let session = self
            .session_for(tab)
            .ok_or_else(|| hexaris_engine::Error::Input("The source tab is closed.".into()))?;
        let function = session.analysis.functions.get(&address).ok_or_else(|| {
            hexaris_engine::Error::Input("Select an analyzed function first.".into())
        })?;
        let context = FunctionContext::build(
            &session.project,
            function,
            &session.references,
            &session.analysis.notices,
            offset,
            CONTEXT_PAGE,
        )?;
        Ok(ContextDialog {
            tab,
            markdown: context.markdown()?,
            json: serde_json::to_string_pretty(&context)?,
            context,
            preview_json: false,
        })
    }

    pub(super) fn tools_window(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.tools_dialog.take() else {
            return;
        };
        let title = match dialog {
            ToolsDialog::Context(_) => "Function context",
            ToolsDialog::Review(_) => "Review assistant response",
            ToolsDialog::Compare(_) => "Compare builds",
        };
        let mut open = true;
        egui::Window::new(title)
            .open(&mut open)
            .default_width(690.0)
            .max_width((ctx.content_rect().width() - 40.0).max(280.0))
            .default_pos(egui::pos2(90.0, 94.0))
            .show(ctx, |ui| match &mut dialog {
                ToolsDialog::Context(context) => self.context_contents(ui, context),
                ToolsDialog::Review(review) => self.review_contents(ui, review),
                ToolsDialog::Compare(comparison) => self.comparison_contents(ui, comparison),
            });
        if open {
            self.tools_dialog = Some(dialog);
        }
    }

    fn context_contents(&mut self, ui: &mut Ui, dialog: &mut ContextDialog) {
        let context = &dialog.context;
        ui.heading(format!("{} · {}", context.name, context.address));
        let count = context.instructions.len();
        if count == 0 {
            ui.label("No decoded instructions.");
        } else {
            ui.label(format!(
                "Instructions {}–{} of {}",
                context.instruction_offset + 1,
                context.instruction_offset + count,
                context.instruction_count
            ));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Copy Markdown").clicked() {
                ui.ctx().copy_text(dialog.markdown.clone());
            }
            if ui.button("Copy JSON").clicked() {
                ui.ctx().copy_text(dialog.json.clone());
            }
            ui.checkbox(&mut dialog.preview_json, "Preview JSON");
        });
        let mut next = None;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(context.instruction_offset > 0, Button::new("Previous page"))
                .clicked()
            {
                next = Some(context.instruction_offset.saturating_sub(CONTEXT_PAGE));
            }
            if ui
                .add_enabled(
                    context.next_instruction_offset.is_some(),
                    Button::new("Next page"),
                )
                .clicked()
            {
                next = context.next_instruction_offset;
            }
        });
        let mut text = if dialog.preview_json {
            dialog.json.as_str()
        } else {
            dialog.markdown.as_str()
        };
        ScrollArea::both()
            .id_salt((
                "context preview",
                context.instruction_offset,
                dialog.preview_json,
            ))
            .max_height(420.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
            });
        if let Some(offset) = next {
            match self.make_context(dialog.tab, context.address.0, offset) {
                Ok(mut updated) => {
                    updated.preview_json = dialog.preview_json;
                    *dialog = updated;
                }
                Err(error) => self.error(error),
            }
        }
    }

    fn review_contents(&mut self, ui: &mut Ui, dialog: &mut ReviewDialog) {
        let Some(session) = self.session_for(dialog.tab) else {
            ui.label("The source tab is closed. Open the image again to review a response.");
            return;
        };
        ui.label(format!("Image: {}", session.project.name()));
        if dialog.response.is_some() {
            if ui.button("Edit response").clicked() {
                dialog.response = None;
                dialog.applied.clear();
                dialog.error = None;
            }
        } else {
            ui.label("Paste the JSON returned by your assistant.");
            ScrollArea::vertical().max_height(128.0).show(ui, |ui| {
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut dialog.input)
                            .code_editor()
                            .char_limit(MAX_RESPONSE_BYTES)
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    )
                    .changed()
                {
                    dialog.error = None;
                }
            });
            if ui
                .add_enabled(
                    !dialog.input.trim().is_empty(),
                    Button::new("Load response"),
                )
                .clicked()
            {
                match AssistantResponse::parse(dialog.input.trim(), &session.project) {
                    Ok(response) => dialog.response = Some(response),
                    Err(error) => dialog.error = Some(error.to_string()),
                }
            }
        }
        if let Some(error) = &dialog.error {
            ui.colored_label(self.theme.palette().warn, error);
        }
        let Some(response) = &dialog.response else {
            return;
        };
        let mut jump = None;
        let mut apply = None;
        ScrollArea::vertical().max_height(390.0).show(ui, |ui| {
            ui.heading("Findings");
            if response.findings.is_empty() {
                ui.label("This response contains no findings.");
            }
            for finding in &response.findings {
                ui.label(&finding.text);
                ui.horizontal_wrapped(|ui| {
                    for &address in &finding.addresses {
                        if evidence_link(ui, session, address) {
                            jump = Some(address.0);
                        }
                    }
                });
            }
            ui.separator();
            ui.heading("Proposed annotations");
            if response.suggestions.is_empty() {
                ui.label("This response contains no annotations.");
            }
            for (index, proposal) in response.suggestions.iter().enumerate() {
                let valid = !matches!(proposal.kind, ProposalKind::FunctionName)
                    || session.analysis.functions.contains_key(&proposal.address.0);
                let mapped = session
                    .project
                    .image()
                    .file_offset(proposal.address.0)
                    .is_some();
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(proposal.kind.label());
                        if evidence_link(ui, session, proposal.address) {
                            jump = Some(proposal.address.0);
                        }
                    });
                    let current = proposal.current(&session.project).or_else(|| {
                        matches!(proposal.kind, ProposalKind::FunctionName)
                            .then(|| session.analysis.functions.get(&proposal.address.0))
                            .flatten()
                            .map(|function| function.name.as_str())
                    });
                    ui.label(format!("Current: {}", current.unwrap_or("—")));
                    ui.label(format!("Proposed: {}", proposal.value));
                    if !proposal.rationale.is_empty() {
                        ui.label(&proposal.rationale);
                    }
                    if dialog.applied.contains(&index) {
                        ui.label("Applied");
                    } else if !valid || !mapped {
                        ui.label("This proposal has no matching annotation target.");
                    } else if ui.button("Apply this annotation").clicked() {
                        apply = Some(index);
                    }
                });
            }
        });
        if let Some(address) = jump {
            self.jump_in_tab(ui.ctx(), dialog.tab, address);
        }
        if let Some(index) = apply {
            match self.apply_proposal(ui.ctx(), dialog.tab, &response.suggestions[index]) {
                Ok(()) => {
                    dialog.error = None;
                    dialog.applied.insert(index);
                }
                Err(error) => dialog.error = Some(error.to_string()),
            }
        }
    }

    fn apply_proposal(
        &mut self,
        ctx: &Context,
        tab: u64,
        proposal: &AnnotationProposal,
    ) -> hexaris_engine::Result<()> {
        self.activate(ctx, tab);
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| hexaris_engine::Error::Input("The source tab is closed.".into()))?;
        proposal.apply(&mut session.project)?;
        session.dirty = true;
        let address = proposal.address.0;
        let text = proposal.current(&session.project).unwrap_or_default();
        match proposal.kind {
            ProposalKind::FunctionName if self.state.selected_function == Some(address) => {
                self.state.name = text.into();
            }
            ProposalKind::Bookmark if self.state.selected_address == Some(address) => {
                self.state.bookmark = text.into();
            }
            ProposalKind::Comment if self.state.selected_address == Some(address) => {
                self.state.note = text.into();
            }
            _ => {}
        }
        self.message = Some((
            false,
            format!(
                "Applied {} at {}",
                proposal.kind.label().to_lowercase(),
                proposal.address
            ),
        ));
        Ok(())
    }

    fn jump_in_tab(&mut self, ctx: &Context, tab: u64, address: u64) {
        if self.session_for(tab).is_some() {
            self.activate(ctx, tab);
            self.goto(address);
        }
    }

    fn comparison_contents(&mut self, ui: &mut Ui, dialog: &mut ComparisonDialog) {
        let tabs: Vec<_> = self
            .background
            .iter()
            .map(|(&id, tab)| (id, tab.session.project.name()))
            .chain(
                self.session
                    .iter()
                    .map(|session| (self.version, session.project.name())),
            )
            .collect();
        let before = dialog.before;
        let after = dialog.after;
        ui.horizontal_wrapped(|ui| {
            tab_picker(ui, "Before", &tabs, &mut dialog.before);
            tab_picker(ui, "After", &tabs, &mut dialog.after);
        });
        let valid = dialog.before != dialog.after
            && tabs.iter().any(|(id, _)| *id == dialog.before)
            && tabs.iter().any(|(id, _)| *id == dialog.after);
        if (before, after) != (dialog.before, dialog.after) || !valid {
            dialog.report = None;
            dialog.error = None;
        }
        if ui.add_enabled(valid, Button::new("Compare")).clicked() {
            match self.compare_tabs(dialog.before, dialog.after) {
                Ok(report) => {
                    dialog.report = Some(report);
                    dialog.error = None;
                }
                Err(error) => {
                    dialog.report = None;
                    dialog.error = Some(error.to_string());
                }
            }
        }
        if let Some(error) = &dialog.error {
            ui.colored_label(self.theme.palette().warn, error);
        }
        let Some(report) = &dialog.report else { return };
        ui.horizontal_wrapped(|ui| {
            if ui.button("Copy Markdown").clicked() {
                ui.ctx().copy_text(report.markdown());
            }
            if ui.button("Copy JSON").clicked() {
                match serde_json::to_string_pretty(report) {
                    Ok(json) => ui.ctx().copy_text(json),
                    Err(error) => self.error(error),
                }
            }
            ui.checkbox(&mut dialog.show_unchanged, "Show unchanged");
        });
        for notice in &report.notices {
            ui.label(notice);
        }
        let visible: Vec<_> = report
            .changes
            .iter()
            .filter(|change| dialog.show_unchanged || change.kind != ChangeKind::Unchanged)
            .collect();
        ui.label(format!(
            "{} of {} function rows",
            visible.len(),
            report.changes.len()
        ));
        let mut jump = None;
        let column_width = ((ui.available_width() - 235.0) / 2.0).max(110.0);
        ui.horizontal(|ui| {
            for (width, title) in [
                (78.0, "Status"),
                (column_width, "Before function"),
                (column_width, "After function"),
                (112.0, "Match basis"),
            ] {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 22.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_width(width);
                        ui.strong(title);
                    },
                );
            }
        });
        ScrollArea::vertical()
            .max_height(400.0)
            .show_rows(ui, 26.0, visible.len(), |ui, rows| {
                for change in &visible[rows] {
                    ui.horizontal(|ui| {
                        let state =
                            ui.add_sized([78.0, 22.0], egui::Label::new(change.kind.label()));
                        if let Some(difference) = &change.first_difference {
                            state.on_hover_text(difference.summary());
                        }
                        for (tab, entry) in [
                            (dialog.before, &change.before),
                            (dialog.after, &change.after),
                        ] {
                            ui.allocate_ui_with_layout(
                                egui::vec2(column_width, 22.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(column_width);
                                    if let Some(entry) = entry {
                                        let text = format!("{} {}", entry.address, entry.name);
                                        if ui
                                            .add(
                                                Button::new(
                                                    RichText::new(&text)
                                                        .color(ui.visuals().hyperlink_color)
                                                        .underline(),
                                                )
                                                .frame(false)
                                                .truncate(),
                                            )
                                            .on_hover_text(&text)
                                            .clicked()
                                        {
                                            jump = Some((tab, entry.address.0));
                                        }
                                    } else {
                                        ui.label("—");
                                    }
                                },
                            );
                        }
                        if let Some(basis) = change.basis {
                            ui.label(RichText::new(basis.label()).weak());
                        }
                    });
                }
            });
        if let Some((tab, address)) = jump {
            self.jump_in_tab(ui.ctx(), tab, address);
        }
    }

    pub(super) fn compare_tabs(
        &self,
        before: u64,
        after: u64,
    ) -> hexaris_engine::Result<BuildComparison> {
        if let Some(before) = self.session_for(before)
            && let Some(after) = self.session_for(after)
        {
            compare_builds(
                &before.project,
                &before.analysis,
                &after.project,
                &after.analysis,
            )
        } else {
            Err(hexaris_engine::Error::Input(
                "Open both builds first.".into(),
            ))
        }
    }
}

fn evidence_link(ui: &mut Ui, session: &Session, address: ContextAddress) -> bool {
    if session.project.image().file_offset(address.0).is_some() {
        ui.link(address.to_string()).clicked()
    } else {
        ui.label(RichText::new(format!("{address} (unmapped)")).weak());
        false
    }
}

fn tab_picker(ui: &mut Ui, label: &str, tabs: &[(u64, &str)], selected: &mut u64) {
    let selection = tabs
        .iter()
        .find(|(id, _)| id == selected)
        .map_or("Choose a tab", |(_, name)| *name);
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!("{label}: {selection}"))
        .show_ui(ui, |ui| {
            for &(id, name) in tabs {
                ui.selectable_value(selected, id, name);
            }
        });
}
