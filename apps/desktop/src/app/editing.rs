use std::path::Path;

use hexaris_engine::{BranchEdit, BytePatch, Instruction, parse_address, write_patched_copy};

use super::{App, Button, Context, FileDialog, RichText, ScrollArea, egui, views::hex_bytes};

const MAX_PENDING: usize = 128;

pub(super) fn branch_menu(
    ui: &mut super::Ui,
    instruction: &Instruction,
) -> Option<(BytePatch, String)> {
    BytePatch::invert_branch(instruction)?;
    for (edit, label, effect) in [
        (
            BranchEdit::Invert,
            "Invert branch…",
            "Invert the condition and keep the branch destination.",
        ),
        (
            BranchEdit::AlwaysTaken,
            "Always take branch…",
            "Jump to the original destination on every path.",
        ),
        (
            BranchEdit::AlwaysFallthrough,
            "Always fall through…",
            "Continue at the next instruction on every path.",
        ),
    ] {
        if ui.button(label).clicked() {
            ui.close();
            return BytePatch::edit_branch(instruction, edit).map(|patch| {
                (
                    patch,
                    format!(
                        "{} at 0x{:X}. {effect}",
                        instruction.text, instruction.address
                    ),
                )
            });
        }
    }
    None
}

#[derive(Default)]
pub(super) struct PatchDialog {
    offset: String,
    replacement: String,
    description: Option<String>,
    error: Option<String>,
}

impl App {
    pub(super) fn open_patch(&mut self, offset: Option<usize>) {
        self.patch_dialog = Some(PatchDialog {
            offset: format!("0x{:X}", offset.unwrap_or(0)),
            ..Default::default()
        });
    }

    pub(super) fn open_branch_patch(&mut self, patch: BytePatch, description: String) {
        self.patch_dialog = Some(PatchDialog {
            offset: format!("0x{:X}", patch.offset),
            replacement: hex_bytes(patch.replacement()),
            description: Some(description),
            error: None,
        });
    }

    pub(super) fn patch_window(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.patch_dialog.take() else {
            return;
        };
        let Some(session) = &mut self.session else {
            return;
        };
        let image = session.project.image();
        let mut open = true;
        let mut export = false;
        let offset = parse_address(dialog.offset.trim())
            .ok()
            .and_then(|value| usize::try_from(value).ok());
        let draft =
            offset.map(|offset| BytePatch::for_replacement(image, offset, &dialog.replacement));
        egui::Window::new("Patch bytes")
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label("Queue fixed-length edits and export a separate binary. Hexaris opens the copy for reanalysis.");
                ui.horizontal(|ui| {
                    ui.label("File offset");
                    if ui.add(egui::TextEdit::singleline(&mut dialog.offset).code_editor().char_limit(32)).changed() {
                        dialog.description = None;
                        dialog.error = None;
                    }
                });
                ui.label("Replacement bytes");
                if ui.add(egui::TextEdit::singleline(&mut dialog.replacement)
                    .code_editor().char_limit(12_288).desired_width(f32::INFINITY)
                    .hint_text("Enter hex bytes, for example 48 8B")).changed() {
                    dialog.description = None;
                    dialog.error = None;
                }
                if let Some(description) = &dialog.description {
                    ui.label(description);
                }
                match &draft {
                    Some(Ok(patch)) => {
                        ui.monospace(format!("Original   {}", preview_bytes(patch.expected())));
                        ui.monospace(format!("Replace    {}", preview_bytes(patch.replacement())));
                    }
                    Some(Err(error)) if !dialog.replacement.trim().is_empty() => {
                        ui.label(RichText::new(error.to_string()).weak());
                    }
                    _ => {
                        if let Some(bytes) = offset.and_then(|offset| image.bytes().get(offset..))
                            .filter(|bytes| !bytes.is_empty())
                        {
                            ui.monospace(format!("Bytes at offset   {}", preview_bytes(bytes)));
                        }
                    }
                }
                if let Some(error) = &dialog.error {
                    ui.colored_label(self.theme.palette().warn, error);
                }
                // Reparse on click so a same-frame text edit never stages the previous preview.
                if ui.add_enabled(!dialog.replacement.trim().is_empty(), Button::new("Queue change")).clicked() {
                    let parsed = parse_address(dialog.offset.trim()).ok()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| "Enter a valid file offset.".to_owned())
                        .and_then(|offset| BytePatch::for_replacement(image, offset, &dialog.replacement)
                            .map_err(|error| error.to_string()));
                    let result = parsed.and_then(|patch| {
                        if session.patches.len() >= MAX_PENDING {
                            return Err("Export the pending patches before queuing more.".into());
                        }
                        if session.patches.iter().any(|existing| existing.overlaps(&patch)) {
                            return Err("This change overlaps a queued patch. Remove that patch first.".into());
                        }
                        session.patches.push(patch);
                        session.patches.sort_unstable_by_key(|patch| patch.offset);
                        Ok(())
                    });
                    match result {
                        Ok(()) => {
                            dialog.replacement.clear();
                            dialog.description = None;
                            dialog.error = None;
                        }
                        Err(error) => dialog.error = Some(error),
                    }
                }
                ui.separator();
                ui.heading(format!("Pending patches ({})", session.patches.len()));
                let mut remove = None;
                ScrollArea::vertical().max_height(215.0).show(ui, |ui| {
                    for (index, patch) in session.patches.iter().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.monospace(format!("0x{:X}   {} → {}", patch.offset,
                                preview_bytes(patch.expected()), preview_bytes(patch.replacement())));
                            if ui.small_button("Remove").clicked() {
                                remove = Some(index);
                            }
                        });
                    }
                });
                if let Some(index) = remove {
                    session.patches.remove(index);
                    dialog.error = None;
                }
                export = ui.add_enabled(
                    !session.patches.is_empty() && self.opening.is_none(),
                    Button::new("Export and open patched copy…"),
                ).clicked();
            });
        if export {
            let name = Path::new(session.project.name());
            let stem = name
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("binary");
            let extension = name
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("bin");
            if let Some(path) = FileDialog::new()
                .set_title("Export a patched binary copy")
                .set_file_name(format!("{stem}.patched.{extension}"))
                .save_file()
            {
                let result = if super::is_project(&path) {
                    Err(hexaris_engine::Error::Input(
                        "Use a binary filename for the patched copy, such as .exe or .bin.".into(),
                    ))
                } else {
                    write_patched_copy(image, &session.patches, &path)
                };
                match result {
                    Ok(()) => {
                        let mode = image.mode;
                        session.patches.clear();
                        self.enqueue_files(ctx, vec![path], mode);
                        return;
                    }
                    Err(error) => dialog.error = Some(error.to_string()),
                }
            }
        }
        if open {
            self.patch_dialog = Some(dialog);
        }
    }
}

fn preview_bytes(bytes: &[u8]) -> String {
    let preview = hex_bytes(&bytes[..bytes.len().min(12)]);
    if bytes.len() > 12 {
        format!("{preview} … ({} bytes)", bytes.len())
    } else {
        preview
    }
}
