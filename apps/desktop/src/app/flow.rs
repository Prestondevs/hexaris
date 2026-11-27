use std::collections::{BTreeMap, VecDeque};

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use hexaris_engine::{
    Block, BytePatch, Edge, EdgeKind, Function, Instruction, NumberBase, Project, ReferenceKind,
    string_at,
};

use super::{Ui, label, selection::AddressSelection, theme};

const CARD: Vec2 = vec2(340.0, 150.0);
const STRIDE: Vec2 = vec2(392.0, 214.0);
const COLUMNS: usize = 5;
const LINE_Y: f32 = 28.0;
const LINE_HEIGHT: f32 = 15.0;

#[derive(Default)]
pub(super) struct GraphState {
    function: Option<u64>,
    layout: Layout,
    zoom: f32,
    pan: Vec2,
    strings: BTreeMap<u64, Vec<(u64, String)>>,
    show_details: bool,
    edge: Option<(usize, Edge)>,
    menu_block: Option<usize>,
    menu_address: Option<u64>,
    pub(super) focus_selection: bool,
    box_anchor: Option<Pos2>,
}

#[derive(Default)]
struct Layout {
    positions: Vec<Pos2>,
    index: BTreeMap<u64, usize>,
    size: Vec2,
}

impl Layout {
    fn new(function: &Function) -> Self {
        let index: BTreeMap<_, _> = function
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.address, index))
            .collect();
        let mut levels = Vec::<Vec<usize>>::new();
        let mut discovered = vec![false; function.blocks.len()];
        // Start with the real entry, then arrange any disconnected decoded blocks.
        for root in index
            .get(&function.address)
            .copied()
            .into_iter()
            .chain(0..function.blocks.len())
        {
            if discovered[root] {
                continue;
            }
            let start = levels.len();
            let mut queue = VecDeque::from([(root, start)]);
            discovered[root] = true;
            while let Some((block, level)) = queue.pop_front() {
                if level == levels.len() {
                    levels.push(Vec::new());
                }
                levels[level].push(block);
                for edge in &function.blocks[block].successors {
                    if let Some(&target) = index.get(&edge.target)
                        && !discovered[target]
                    {
                        discovered[target] = true;
                        queue.push_back((target, level + 1));
                    }
                }
            }
        }
        let columns = levels.iter().map(Vec::len).max().unwrap_or(1).min(COLUMNS);
        let mut positions = vec![Pos2::ZERO; function.blocks.len()];
        let mut row = 0_usize;
        for level in levels {
            for chunk in level.chunks(COLUMNS) {
                let centered = (columns - chunk.len()) as f32 * STRIDE.x / 2.0;
                for (column, &block) in chunk.iter().enumerate() {
                    positions[block] = pos2(
                        28.0 + centered + column as f32 * STRIDE.x,
                        24.0 + row as f32 * STRIDE.y,
                    );
                }
                row += 1;
            }
        }
        Self {
            positions,
            index,
            size: vec2(
                (columns - 1) as f32 * STRIDE.x + CARD.x + 57.0,
                row.saturating_sub(1) as f32 * STRIDE.y + CARD.y + 49.0,
            ),
        }
    }
}

pub(super) enum GraphAction {
    Select(u64),
    Disassemble(u64),
    Follow(u64),
    Patch(usize),
    Highlight(u64, u64),
    Invert(BytePatch, String),
}

pub(super) fn graph(
    ui: &mut Ui,
    state: &mut GraphState,
    function: &Function,
    project: &Project,
    selected: Option<u64>,
    selection: &AddressSelection,
    number_base: NumberBase,
) -> Option<GraphAction> {
    if function.blocks.is_empty() {
        label(ui, "No decoded blocks for this function.");
        return None;
    }
    let changed = state.function != Some(function.address);
    if changed {
        state.function = Some(function.address);
        state.layout = Layout::new(function);
        state.edge = None;
        state.menu_block = None;
        state.menu_address = None;
        state.strings.clear();
        for reference in &function.references {
            if reference.kind == ReferenceKind::Address
                && let Some(text) = string_at(project.image(), reference.target)
            {
                state
                    .strings
                    .entry(reference.source)
                    .or_default()
                    .push((reference.target, text));
            }
        }
    }
    let palette = theme::current(ui.ctx());
    let selected_index = selected
        .and_then(|address| {
            let instruction = function
                .instructions
                .binary_search_by_key(&address, |instruction| instruction.address)
                .ok()?;
            function.blocks.iter().position(|block| {
                (block.first_instruction..block.first_instruction + block.instruction_count)
                    .contains(&instruction)
            })
        })
        .or_else(|| state.layout.index.get(&function.address).copied());
    let mut fit = changed;
    let mut focus = std::mem::take(&mut state.focus_selection)
        .then_some(selected_index)
        .flatten();
    let mut change_zoom = 1.0;
    let mut result = None;
    ui.horizontal_wrapped(|ui| {
        fit |= ui.button("Fit all blocks").clicked();
        if ui
            .add_enabled(selected_index.is_some(), egui::Button::new("Focus block"))
            .clicked()
        {
            focus = selected_index;
        }
        if ui.small_button("−").clicked() {
            change_zoom = 0.8;
        }
        if ui.small_button("+").clicked() {
            change_zoom = 1.25;
        }
        ui.label(format!(
            "{:.1}% · {} blocks",
            state.zoom * 100.0,
            function.blocks.len()
        ));
        fit |= ui
            .toggle_value(&mut state.show_details, "Block details")
            .changed();
        if let Some(index) = selected_index {
            let block = &function.blocks[index];
            if ui.button("Open disassembly").clicked() {
                result = Some(GraphAction::Disassemble(block.address));
            }
            let terminal = block_instructions(function, block)
                .last()
                .expect("nonempty decoded block");
            if let Some(action) = invert_action(terminal)
                && ui
                    .button("Invert branch…")
                    .on_hover_text(&terminal.text)
                    .clicked()
            {
                result = Some(action);
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        label(
            ui,
            "Drag to pan · Shift+drag to highlight · Right-click for actions",
        );
        ui.colored_label(palette.accent, "Taken");
        ui.colored_label(palette.text, "Fallthrough");
        ui.colored_label(palette.warn, "Jump");
    });
    if state.show_details
        && let Some(index) = selected_index
    {
        let block = &function.blocks[index];
        let detail_action = details(ui, state, block, function, project, selected);
        result = detail_action.or(result);
    }
    let size = ui.available_size().max(vec2(120.0, 120.0));
    let (canvas, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let min_zoom =
        (0.77 * (size.x / state.layout.size.x).min(size.y / state.layout.size.y)).min(0.25);
    if fit {
        state.zoom =
            (0.94 * (size.x / state.layout.size.x).min(size.y / state.layout.size.y)).min(1.0);
        state.pan = (size - state.layout.size * state.zoom) * 0.5;
        ui.ctx().request_repaint();
    }
    if canvas.drag_started_by(egui::PointerButton::Primary) {
        state.box_anchor = ui.input(|input| {
            input
                .modifiers
                .shift
                .then(|| input.pointer.press_origin())
                .flatten()
        });
    }
    if canvas.dragged_by(egui::PointerButton::Primary) && state.box_anchor.is_none() {
        state.pan += canvas.drag_delta();
    }
    let pointer = canvas.hover_pos().map(|point| point - canvas.rect.min);
    let scroll = if canvas.hovered() {
        ui.input(|input| input.smooth_scroll_delta.y)
    } else {
        0.0
    };
    if scroll != 0.0 || change_zoom != 1.0 {
        let pivot = pointer.unwrap_or(size * 0.5);
        let zoom = (state.zoom * change_zoom * (scroll / 360.0).exp()).clamp(min_zoom, 3.5);
        state.pan = pivot + (state.pan - pivot) * (zoom / state.zoom);
        state.zoom = zoom;
        if scroll != 0.0 {
            ui.input_mut(|input| input.smooth_scroll_delta.y = 0.0);
        }
    }
    if (canvas.clicked() || canvas.double_clicked())
        && let Some(at) = canvas.interact_pointer_pos()
    {
        let world = (at - canvas.rect.min - state.pan) / state.zoom;
        if let Some(index) = state.layout.positions.iter().position(|&position| {
            Rect::from_min_size(position, CARD).contains(pos2(world.x, world.y))
        }) {
            let block = &function.blocks[index];
            let instructions = block_instructions(function, block);
            let instruction = at_preview_row(
                instructions,
                world.y - state.layout.positions[index].y,
                state.zoom,
            );
            state.edge = None;
            result = Some(GraphAction::Select(
                instruction.map_or(block.address, |item| item.address),
            ));
            if canvas.double_clicked() {
                focus = Some(index);
            }
        }
    }
    if let Some(index) = focus {
        state.zoom = (size.x / (CARD.x * 1.6)).clamp(0.45, 1.25);
        state.pan =
            size * 0.5 - (state.layout.positions[index].to_vec2() + CARD * 0.5) * state.zoom;
        ui.ctx().request_repaint();
    }
    painter.rect_filled(canvas.rect, 4, palette.bg);
    let zoom = state.zoom;
    let locate = |world: Pos2| canvas.rect.min + state.pan + world.to_vec2() * zoom;
    let dimensions = CARD * zoom;
    let box_selection = state
        .box_anchor
        .zip(ui.input(|input| input.pointer.interact_pos()))
        .map(|(anchor, point)| Rect::from_two_pos(anchor, point).intersect(canvas.rect));
    if canvas.drag_stopped_by(egui::PointerButton::Primary) {
        state.box_anchor = None;
        if let Some(rect) = box_selection {
            let mut first = u64::MAX;
            let mut last = 0;
            for (index, block) in function.blocks.iter().enumerate() {
                if rect.intersects(Rect::from_min_size(
                    locate(state.layout.positions[index]),
                    dimensions,
                )) {
                    let terminal = &function.instructions
                        [block.first_instruction + block.instruction_count - 1];
                    first = first.min(block.address);
                    last = last.max(terminal.address);
                }
            }
            if first <= last {
                result = Some(GraphAction::Highlight(first, last));
            }
        }
    }
    let graph_pointer = canvas
        .interact_pointer_pos()
        .or_else(|| canvas.hover_pos())
        .filter(|&at| canvas.rect.contains(at));
    let on_card =
        graph_pointer.and_then(|at| {
            state.layout.positions.iter().position(|&position| {
                Rect::from_min_size(locate(position), dimensions).contains(at)
            })
        });
    let edge_pointer = graph_pointer.filter(|_| on_card.is_none());
    let mut nearest_edge = None;
    for (index, block) in function.blocks.iter().enumerate() {
        let source = locate(state.layout.positions[index]);
        for edge in &block.successors {
            let port = match edge.kind {
                EdgeKind::Taken => 0.35,
                EdgeKind::Fallthrough => 0.65,
                EdgeKind::Jump => 0.5,
            };
            let from = source + vec2(dimensions.x * port, dimensions.y);
            let color = match edge.kind {
                EdgeKind::Taken => palette.accent,
                EdgeKind::Fallthrough => palette.text.gamma_multiply(0.72),
                EdgeKind::Jump => palette.warn,
            };
            let stroke = Stroke::new((1.6 * zoom).clamp(0.7, 2.8), color);
            let Some(&target) = state.layout.index.get(&edge.target) else {
                if canvas
                    .rect
                    .intersects(Rect::from_min_size(source, dimensions))
                {
                    let shift = (port - 0.5) * 80.0 * zoom;
                    let arrow = vec2(shift, 14.0 * zoom);
                    record_edge(
                        edge_pointer,
                        &[from, from + arrow],
                        (index, *edge),
                        &mut nearest_edge,
                    );
                    painter.arrow(from, arrow, stroke);
                    if zoom >= 0.57 {
                        let align = if port < 0.5 {
                            Align2::RIGHT_TOP
                        } else {
                            Align2::LEFT_TOP
                        };
                        painter.text(
                            from + arrow,
                            align,
                            format!("0x{:X}", edge.target),
                            FontId::monospace(11.0 * zoom),
                            color,
                        );
                    }
                }
                continue;
            };
            let target = locate(state.layout.positions[target]);
            let to = target + vec2(dimensions.x * (0.5 + (port - 0.5) * 0.6), 0.0);
            let next_row = to.y > from.y && to.y - source.y < STRIDE.y * zoom * 1.1;
            let bend = if next_row {
                (from.y + to.y) / 2.0
            } else {
                from.y + 20.0 * zoom
            };
            let lane = if next_row {
                to.x
            } else {
                locate(pos2(state.layout.size.x - 8.0, 0.0)).x
            };
            let end = to - vec2(0.0, (8.0 * zoom).max(1.0));
            let points = [
                from,
                pos2(from.x, bend),
                pos2(lane, bend),
                pos2(lane, end.y),
                end,
            ];
            if !Rect::from_points(&points)
                .expand(2.0)
                .intersects(canvas.rect)
            {
                continue;
            }
            record_edge(edge_pointer, &points, (index, *edge), &mut nearest_edge);
            let stroke = if state.edge == Some((index, *edge)) {
                Stroke::new(stroke.width + 2.0, color)
            } else {
                stroke
            };
            painter.add(egui::Shape::line(points.to_vec(), stroke));
            if zoom >= 0.16 {
                painter.arrow(end, to - end, stroke);
            }
        }
    }
    let hit = nearest_edge.map(|(_, edge)| edge);
    if hit.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if let Some((index, edge)) = hit
        && (canvas.clicked() || canvas.secondary_clicked())
    {
        state.edge = Some((index, edge));
        let terminal = block_instructions(function, &function.blocks[index])
            .last()
            .expect("nonempty decoded block");
        result = Some(GraphAction::Select(terminal.address));
    }
    if canvas.secondary_clicked() {
        state.menu_block = hit.map(|(index, _)| index).or(on_card);
        state.menu_address = on_card
            .zip(canvas.interact_pointer_pos())
            .and_then(|(index, at)| {
                let top = locate(state.layout.positions[index]).y;
                let block = &function.blocks[index];
                at_preview_row(
                    block_instructions(function, block),
                    (at.y - top) / zoom,
                    zoom,
                )
            })
            .map(|instruction| instruction.address);
        if on_card.is_some() {
            state.edge = None;
        }
    }
    canvas.context_menu(|ui| {
        let Some(index) = state.menu_block else {
            label(ui, "Select a block or branch.");
            return;
        };
        let block = &function.blocks[index];
        let instructions = block_instructions(function, block);
        let instruction = state
            .menu_address
            .and_then(|address| {
                instructions
                    .binary_search_by_key(&address, |instruction| instruction.address)
                    .ok()
                    .map(|index| &instructions[index])
            })
            .unwrap_or_else(|| instructions.last().expect("nonempty decoded block"));
        if let Some((_, edge)) = state.edge
            && ui
                .button(format!(
                    "Follow {} -> 0x{:X}",
                    edge_name(edge.kind),
                    edge.target
                ))
                .clicked()
        {
            result = Some(follow_action(state, edge.target));
            ui.close();
        }
        if let Some((patch, description)) = super::editing::branch_menu(ui, instruction) {
            result = Some(GraphAction::Invert(patch, description));
        }
        if ui.button("Patch bytes here…").clicked() {
            result = Some(GraphAction::Patch(instruction.file_offset));
            ui.close();
        }
        if ui.button("Open block in disassembly").clicked() {
            result = Some(GraphAction::Disassemble(block.address));
            ui.close();
        }
    });
    for (index, block) in function.blocks.iter().enumerate() {
        let rect = Rect::from_min_size(locate(state.layout.positions[index]), dimensions);
        if !canvas.rect.intersects(rect) {
            continue;
        }
        let terminal =
            &function.instructions[block.first_instruction + block.instruction_count - 1];
        let highlight = selected_index == Some(index)
            || selection
                .range
                .is_some_and(|(first, last)| first <= terminal.address && block.address <= last);
        painter.rect(
            rect,
            4,
            if highlight {
                ui.visuals().selection.bg_fill
            } else {
                palette.raised
            },
            Stroke::new(
                if highlight { 2.0 } else { 1.0_f32 },
                if highlight {
                    palette.accent
                } else {
                    palette.border
                },
            ),
            StrokeKind::Inside,
        );
        if zoom < 0.3 {
            continue;
        }
        let card = painter.with_clip_rect(rect.shrink(2.0));
        card.text(
            rect.min + vec2(10.0, 8.0) * zoom,
            Align2::LEFT_TOP,
            format!(
                "0x{:X}   ·   {} instructions",
                block.address, block.instruction_count
            ),
            FontId::monospace(13.0 * zoom),
            palette.accent,
        );
        if zoom >= 0.57 {
            let instructions = block_instructions(function, block);
            let rows = instructions.len().min(6);
            for row in 0..rows {
                let at = rect.min + vec2(10.0, LINE_Y + row as f32 * LINE_HEIGHT) * zoom;
                if let Some(instruction) = preview_row(instructions, row) {
                    let mut job = egui::text::LayoutJob::default();
                    instruction.visit_tokens_in(number_base, |text, kind| {
                        job.append(
                            text,
                            0.0,
                            egui::TextFormat {
                                font_id: FontId::monospace(12.0 * zoom),
                                color: super::views::syntax_color(palette, kind),
                                ..Default::default()
                            },
                        )
                    });
                    card.galley(
                        at,
                        ui.fonts_mut(|fonts| fonts.layout_job(job)),
                        palette.text,
                    );
                } else {
                    card.text(
                        at,
                        Align2::LEFT_TOP,
                        "…",
                        FontId::monospace(12.0 * zoom),
                        palette.muted,
                    );
                }
            }
            if let Some((_, text)) = instructions.iter().find_map(|item| {
                state
                    .strings
                    .get(&item.address)
                    .and_then(|strings| strings.first())
            }) {
                let text: String = text.chars().take(44).collect();
                card.text(
                    rect.min + vec2(10.0, 126.0) * zoom,
                    Align2::LEFT_TOP,
                    format!("{text:?}"),
                    FontId::monospace(12.0 * zoom),
                    palette.register,
                );
            }
        }
    }
    if let Some(rect) = box_selection {
        painter.rect(
            rect,
            2,
            palette.accent.gamma_multiply(0.16),
            Stroke::new(1.0_f32, palette.accent),
            StrokeKind::Inside,
        );
    }
    result
}

fn block_instructions<'a>(function: &'a Function, block: &Block) -> &'a [Instruction] {
    &function.instructions
        [block.first_instruction..block.first_instruction + block.instruction_count]
}

fn preview_row(instructions: &[Instruction], row: usize) -> Option<&Instruction> {
    if instructions.len() <= 6 || row < 4 {
        instructions.get(row)
    } else if row == 5 {
        instructions.last()
    } else {
        None
    }
}

fn at_preview_row(instructions: &[Instruction], offset: f32, zoom: f32) -> Option<&Instruction> {
    if zoom < 0.57 || !(LINE_Y..LINE_Y + 6.0 * LINE_HEIGHT).contains(&offset) {
        return None;
    }
    preview_row(instructions, ((offset - LINE_Y) / LINE_HEIGHT) as usize)
}

fn edge_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Taken => "Taken",
        EdgeKind::Fallthrough => "Fallthrough",
        EdgeKind::Jump => "Jump",
    }
}

fn record_edge(
    pointer: Option<Pos2>,
    points: &[Pos2],
    edge: (usize, Edge),
    nearest: &mut Option<(f32, (usize, Edge))>,
) {
    let Some(at) = pointer else { return };
    for segment in points.windows(2) {
        let vector = segment[1] - segment[0];
        let length = vector.length_sq();
        if length == 0.0 {
            continue;
        }
        let position = ((at - segment[0]).dot(vector) / length).clamp(0.0, 1.0);
        let distance = (at - (segment[0] + vector * position)).length_sq();
        if distance <= 36.0 && nearest.is_none_or(|(prior, _)| distance < prior) {
            *nearest = Some((distance, edge));
        }
    }
}

fn invert_action(terminal: &Instruction) -> Option<GraphAction> {
    let patch = BytePatch::invert_branch(terminal)?;
    Some(GraphAction::Invert(
        patch,
        format!(
            "Invert {} at 0x{:X}. The destination and byte length stay the same.",
            terminal.text, terminal.address
        ),
    ))
}

fn follow_action(state: &GraphState, target: u64) -> GraphAction {
    if state.layout.index.contains_key(&target) {
        GraphAction::Select(target)
    } else {
        GraphAction::Follow(target)
    }
}

fn details(
    ui: &mut Ui,
    state: &GraphState,
    block: &Block,
    function: &Function,
    project: &Project,
    selected: Option<u64>,
) -> Option<GraphAction> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("Block 0x{:X}", block.address));
        for edge in &block.successors {
            if ui
                .button(format!("{} -> 0x{:X}", edge_name(edge.kind), edge.target))
                .clicked()
            {
                action = Some(follow_action(state, edge.target));
            }
        }
    });
    let instructions = block_instructions(function, block);
    let row_height = ui.spacing().interact_size.y;
    egui::ScrollArea::both()
        .max_height(124.0)
        .auto_shrink([false, true])
        .id_salt(("block-details", function.address, block.address))
        .show_rows(ui, row_height, instructions.len(), |ui, rows| {
            for instruction in &instructions[rows] {
                ui.horizontal(|ui| {
                    let text = egui::RichText::new(format!(
                        "0x{:X}   {}",
                        instruction.address, instruction.text
                    ))
                    .monospace();
                    let row = ui.selectable_label(selected == Some(instruction.address), text);
                    if row.clicked() {
                        action = Some(GraphAction::Select(instruction.address));
                    }
                    row.context_menu(|ui| {
                        if let Some((patch, description)) =
                            super::editing::branch_menu(ui, instruction)
                        {
                            action = Some(GraphAction::Invert(patch, description));
                        }
                        if ui.button("Patch bytes here…").clicked() {
                            action = Some(GraphAction::Patch(instruction.file_offset));
                            ui.close();
                        }
                    });
                    if let Some(strings) = state.strings.get(&instruction.address) {
                        for (address, text) in strings {
                            if ui
                                .link(format!("{text:?}"))
                                .on_hover_text(format!("String at 0x{address:X}"))
                                .clicked()
                            {
                                action = Some(GraphAction::Follow(*address));
                            }
                        }
                    }
                    if let Some(comment) = project
                        .comment(instruction.address)
                        .and_then(|comment| comment.lines().next())
                    {
                        label(ui, comment);
                    }
                });
            }
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexaris_engine::{Architecture, Image, LoadMode, analyze_function};

    #[test]
    fn layout_includes_every_block_beyond_the_previous_display_limit() {
        let mut bytes = [0x75, 0].repeat(240);
        bytes.push(0xc3);
        let image = Image::from_bytes(
            bytes,
            LoadMode::Raw {
                architecture: Architecture::X86_64,
                base: 0x1000,
            },
        )
        .unwrap();
        let function = analyze_function(&image, 0x1000).unwrap();
        let layout = Layout::new(&function);
        assert_eq!(function.blocks.len(), 241);
        assert_eq!(layout.positions.len(), 241);
        assert_eq!(layout.index[&0x11e0], 240);
        assert!(
            layout
                .positions
                .windows(2)
                .all(|pair| pair[0].y < pair[1].y)
        );
        assert!(
            layout
                .positions
                .iter()
                .all(|position| position.y + CARD.y < layout.size.y)
        );
    }
}
