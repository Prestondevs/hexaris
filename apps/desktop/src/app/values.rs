use eframe::egui::{self, Context, Ui};
use hexaris_engine::NumberBase;

#[derive(Default)]
pub(super) struct Converter {
    pub(super) open: bool,
    bytes_mode: bool,
    input: String,
    input_base: NumberBase,
}

impl Converter {
    pub(super) fn show(&mut self, ctx: &Context) {
        if !self.open {
            return;
        }
        egui::Window::new("Convert value")
            .open(&mut self.open)
            .default_width(465.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bytes_mode, false, "Integer");
                    ui.selectable_value(&mut self.bytes_mode, true, "Hex bytes");
                    if !self.bytes_mode {
                        base_picker(ui, "Input base", &mut self.input_base);
                    }
                });
                let hint = if self.bytes_mode {
                    "48 65 6c 6c 6f"
                } else {
                    "0xFF, 0b1010, 0o755, or a value in the input base"
                };
                ui.add(
                    egui::TextEdit::singleline(&mut self.input)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .char_limit(4096)
                        .hint_text(hint),
                );
                if self.input.trim().is_empty() {
                    return;
                }
                if self.bytes_mode {
                    match hexaris_engine::parse_bytes(self.input.trim().trim_start_matches("0x")) {
                        Ok(bytes) => {
                            copy_row(
                                ui,
                                "UTF-8",
                                &format!("{:?}", String::from_utf8_lossy(&bytes)),
                            );
                            ui.label(format!("{} bytes", bytes.len()));
                        }
                        Err(error) => super::label(ui, &error.to_string()),
                    }
                } else if let Some(value) = parse_integer(&self.input, self.input_base) {
                    copy_row(ui, "Hex", &format!("{value:#X}"));
                    copy_row(ui, "Decimal", &value.to_string());
                    copy_row(ui, "Octal", &format!("{value:#o}"));
                    copy_row(ui, "Binary", &format!("{value:#b}"));
                    copy_row(ui, "Signed 64-bit", &(value as i64).to_string());
                } else {
                    super::label(ui, "Enter a valid integer that fits in 64 bits.");
                }
            });
    }
}

pub(super) fn base_picker(ui: &mut Ui, id: &'static str, base: &mut NumberBase) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(base.label())
        .width(74.0)
        .show_ui(ui, |ui| {
            for candidate in NumberBase::ALL {
                ui.selectable_value(base, candidate, candidate.label());
            }
        });
}

fn copy_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ui.monospace(value);
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(value.to_owned());
        }
    });
}

fn parse_integer(text: &str, base: NumberBase) -> Option<u64> {
    let clean = text.trim().replace('_', "").to_ascii_lowercase();
    let radix = match base {
        NumberBase::Hex => 16,
        NumberBase::Decimal => 10,
        NumberBase::Octal => 8,
        NumberBase::Binary => 2,
    };
    let negative = clean.starts_with('-');
    let digits = clean.strip_prefix(['-', '+']).unwrap_or(&clean);
    let (digits, radix) = if let Some(digits) = digits.strip_prefix("0x") {
        (digits, 16)
    } else if let Some(digits) = digits.strip_prefix("0b") {
        (digits, 2)
    } else if let Some(digits) = digits.strip_prefix("0o") {
        (digits, 8)
    } else {
        (digits, radix)
    };
    let value = u64::from_str_radix(digits, radix).ok()?;
    if negative && value > 1_u64 << 63 {
        return None;
    }
    Some(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}
