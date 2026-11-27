use eframe::egui::{self, Response, Ui};

#[derive(Default)]
pub(super) struct AddressSelection {
    pub(super) range: Option<(u64, u64)>,
    anchor: Option<u64>,
}

impl AddressSelection {
    pub(super) fn set(&mut self, first: u64, last: u64) {
        self.range = Some((first.min(last), first.max(last)));
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn contains(&self, address: u64) -> bool {
        self.range
            .is_some_and(|(start, end)| (start..=end).contains(&address))
    }

    pub(super) fn interact(
        &mut self,
        ui: &Ui,
        row: &Response,
        address: u64,
        selected: Option<u64>,
    ) {
        let (held, shift, pointer) = ui.input(|input| {
            (
                input.pointer.primary_down(),
                input.modifiers.shift,
                input.pointer.interact_pos(),
            )
        });
        if row.clicked() {
            if shift {
                self.set(selected.unwrap_or(address), address);
            } else {
                self.clear();
            }
        } else if row.drag_started_by(egui::PointerButton::Primary) {
            self.anchor = Some(address);
            self.set(address, address);
        }
        if held
            && let Some(anchor) = self.anchor
            && pointer.is_some_and(|pos| row.rect.intersect(ui.clip_rect()).contains(pos))
        {
            self.set(anchor, address);
            ui.ctx().request_repaint();
        }
        if !held {
            self.anchor = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{
        Button, CentralPanel, Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect,
        Sense, vec2,
    };

    #[test]
    fn dragging_across_instruction_rows_preserves_the_address_range() {
        let ctx = Context::default();
        let mut selection = AddressSelection::default();
        let mut frame = |events| {
            let mut centers = Vec::new();
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(320.0, 240.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    CentralPanel::default().show(ctx, |ui| {
                        for address in [0x1000, 0x1005, 0x1008] {
                            let row = ui.add_sized(
                                [180.0, 24.0],
                                Button::new(format!("0x{address:X}"))
                                    .sense(Sense::click_and_drag()),
                            );
                            centers.push(row.rect.center());
                            selection.interact(ui, &row, address, None);
                        }
                    });
                },
            );
            centers
        };
        let centers = frame(Vec::new());
        let button = |pos, pressed| Event::PointerButton {
            pos,
            pressed,
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        frame(vec![
            Event::PointerMoved(centers[0]),
            button(centers[0], true),
        ]);
        frame(vec![Event::PointerMoved(centers[2])]);
        frame(vec![button(centers[2], false)]);
        assert_eq!(selection.range, Some((0x1000, 0x1008)));
        assert!(selection.contains(0x1005));
    }
}
