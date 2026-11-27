use hexaris_engine::Project;

use super::{App, Context, egui};

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Function,
    Argument(u8),
}

pub(super) struct RenameDialog {
    address: u64,
    arguments: u8,
    target: Target,
    name: String,
    error: Option<String>,
}

impl App {
    pub(super) fn open_rename(&mut self, address: u64, arguments: u8) {
        let Some(session) = &self.session else { return };
        let target = if arguments > 0 {
            Target::Argument(1)
        } else {
            Target::Function
        };
        self.rename = Some(RenameDialog {
            address,
            arguments,
            target,
            name: current_name(&session.project, address, target),
            error: None,
        });
    }

    pub(super) fn rename_window(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.rename.take() else {
            return;
        };
        let Some(session) = &mut self.session else {
            return;
        };
        let mut open = true;
        let mut saved = false;
        egui::Window::new("Rename")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.monospace(format!("Function 0x{:X}", dialog.address));
                if dialog.arguments > 0 {
                    let previous = dialog.target;
                    egui::ComboBox::from_id_salt("rename-target")
                        .selected_text(target_name(dialog.target))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut dialog.target, Target::Function, "Function");
                            for number in 1..=dialog.arguments {
                                ui.selectable_value(
                                    &mut dialog.target,
                                    Target::Argument(number),
                                    target_name(Target::Argument(number)),
                                );
                            }
                        });
                    if previous != dialog.target {
                        dialog.name = current_name(&session.project, dialog.address, dialog.target);
                        dialog.error = None;
                    }
                }
                ui.label("New name");
                let field = ui.add(
                    egui::TextEdit::singleline(&mut dialog.name)
                        .hint_text("Leave blank to restore the discovered name")
                        .char_limit(256),
                );
                if ui.button("Apply name").clicked()
                    || (field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                {
                    let result = match dialog.target {
                        Target::Function => session
                            .project
                            .set_function_name(dialog.address, dialog.name.clone()),
                        Target::Argument(number) => session.project.set_argument_name(
                            dialog.address,
                            number,
                            dialog.name.clone(),
                        ),
                    };
                    match result {
                        Ok(()) => {
                            session.dirty = true;
                            if self.state.selected_function == Some(dialog.address)
                                && dialog.target == Target::Function
                            {
                                self.state.name = current_name(
                                    &session.project,
                                    dialog.address,
                                    Target::Function,
                                );
                            }
                            saved = true;
                        }
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                }
                if let Some(error) = &dialog.error {
                    ui.colored_label(self.theme.palette().warn, error);
                }
            });
        if open && !saved {
            self.rename = Some(dialog);
        }
    }
}

fn current_name(project: &Project, address: u64, target: Target) -> String {
    match target {
        Target::Function => project.function_name(address),
        Target::Argument(number) => project.argument_name(address, number),
    }
    .unwrap_or_default()
    .to_owned()
}

fn target_name(target: Target) -> String {
    match target {
        Target::Function => "Function".to_owned(),
        Target::Argument(number) => format!("Argument arg{number}"),
    }
}

pub(super) fn argument_count(code: &str) -> u8 {
    code.lines()
        .next()
        .unwrap_or_default()
        .matches("uint32_t arg")
        .count()
        .min(6) as u8
}

pub(super) fn named_structured(project: &Project, function: u64, code: &str) -> String {
    let synthetic = format!("sub_{function:X}");
    let function_name = project.function_name(function).filter(|name| {
        name.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
            && name.chars().all(identifier_char)
    });
    let mut output = String::with_capacity(code.len());
    let mut start = 0;
    for (end, ch) in code
        .char_indices()
        .chain(std::iter::once((code.len(), '\0')))
    {
        if identifier_char(ch) {
            continue;
        }
        let token = &code[start..end];
        let alternate = if token == synthetic {
            function_name
        } else {
            token
                .strip_prefix("arg")
                .and_then(|suffix| suffix.parse::<u8>().ok())
                .and_then(|number| project.argument_name(function, number))
        };
        output.push_str(alternate.unwrap_or(token));
        if end < code.len() {
            output.push(ch);
        }
        start = end + ch.len_utf8();
    }
    output
}

fn identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
