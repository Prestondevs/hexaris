use eframe::egui::{
    Button, Context, CursorIcon, Label, PointerButton, ResizeDirection, RichText, Sense, Ui,
    ViewportCommand,
};

pub(super) fn title(ui: &mut Ui) {
    let title = ui.add(
        Label::new(RichText::new("Hexaris").size(21.0).strong()).sense(Sense::click_and_drag()),
    );
    drag(ui.ctx(), &title);
}

pub(super) fn drag_space(ui: &mut Ui) {
    let response = ui.allocate_response(
        eframe::egui::vec2(ui.available_width().max(0.0), ui.spacing().interact_size.y),
        Sense::click_and_drag(),
    );
    drag(ui.ctx(), &response);
}

fn drag(ctx: &Context, response: &eframe::egui::Response) {
    if response.double_clicked() {
        maximize(ctx);
    } else if response.drag_started_by(PointerButton::Primary) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

pub(super) fn controls(ui: &mut Ui) {
    let maximized = ui.input(|input| input.viewport().maximized == Some(true));
    if ui
        .add_sized([29.0, 24.0], Button::new("×"))
        .on_hover_text("Close window")
        .clicked()
    {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }
    if ui
        .add_sized([29.0, 24.0], Button::new(if maximized { "❐" } else { "□" }))
        .on_hover_text(if maximized {
            "Restore window"
        } else {
            "Maximize window"
        })
        .clicked()
    {
        maximize(ui.ctx());
    }
    if ui
        .add_sized([29.0, 24.0], Button::new("−"))
        .on_hover_text("Minimize window")
        .clicked()
    {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
    ui.add_space(6.0);
}

fn maximize(ctx: &Context) {
    let maximized = ctx.input(|input| input.viewport().maximized == Some(true));
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
}

pub(super) fn resize_edges(ctx: &Context) {
    let (pointer, pressed, maximized) = ctx.input(|input| {
        (
            input.pointer.hover_pos(),
            input.pointer.button_pressed(PointerButton::Primary),
            input.viewport().maximized == Some(true),
        )
    });
    let rect = ctx.viewport_rect();
    let Some(pointer) = pointer.filter(|&pos| !maximized && rect.contains(pos)) else {
        return;
    };
    const EDGE: f32 = 5.0;
    let x = if pointer.x < rect.left() + EDGE {
        -1
    } else if pointer.x > rect.right() - EDGE {
        1
    } else {
        0
    };
    let y = if pointer.y < rect.top() + EDGE {
        -1
    } else if pointer.y > rect.bottom() - EDGE {
        1
    } else {
        0
    };
    let (direction, cursor) = match (x, y) {
        (-1, -1) => (ResizeDirection::NorthWest, CursorIcon::ResizeNwSe),
        (1, -1) => (ResizeDirection::NorthEast, CursorIcon::ResizeNeSw),
        (-1, 1) => (ResizeDirection::SouthWest, CursorIcon::ResizeNeSw),
        (1, 1) => (ResizeDirection::SouthEast, CursorIcon::ResizeNwSe),
        (-1, 0) => (ResizeDirection::West, CursorIcon::ResizeHorizontal),
        (1, 0) => (ResizeDirection::East, CursorIcon::ResizeHorizontal),
        (0, -1) => (ResizeDirection::North, CursorIcon::ResizeVertical),
        (0, 1) => (ResizeDirection::South, CursorIcon::ResizeVertical),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if pressed {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}
