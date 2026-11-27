use std::collections::{BTreeMap, VecDeque};

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use hexaris_engine::{BytePatch, EdgeKind, Function};

use super::{Ui, label, theme};

const CARD: Vec2 = vec2(290.0, 128.0);
const STRIDE: Vec2 = vec2(342.0, 192.0);
const COLUMNS: usize = 5;

#[derive(Default)]
pub(super) struct GraphState {
    function: Option<u64>,
    layout: Layout,
    zoom: f32,
    pan: Vec2,
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
    Invert(BytePatch, String),
}

pub(super) fn graph(
    ui: &mut Ui,
    state: &mut GraphState,
    function: &Function,
    selected: Option<u64>,
) -> Option<GraphAction> {
    if function.blocks.is_empty() {
        label(ui, "No decoded blocks for this function.");
        return None;
    }
    let changed = state.function != Some(function.address);
    if changed {
        state.function = Some(function.address);
        state.layout = Layout::new(function);
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
    let mut focus = None;
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
        if let Some(index) = selected_index {
            let block = &function.blocks[index];
            if ui.button("Open disassembly").clicked() {
                result = Some(GraphAction::Disassemble(block.address));
            }
            let terminal =
                &function.instructions[block.first_instruction + block.instruction_count - 1];
            if let Some(patch) = BytePatch::invert_branch(terminal)
                && ui
                    .button("Invert branch…")
                    .on_hover_text(&terminal.text)
                    .clicked()
            {
                result = Some(GraphAction::Invert(
                    patch,
                    format!(
                        "Invert {} at 0x{:X}. The destination and byte length stay the same.",
                        terminal.text, terminal.address
                    ),
                ));
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        label(ui, "Drag to pan · Scroll to zoom · Double-click to focus");
        ui.colored_label(palette.accent, "Taken");
        ui.colored_label(palette.text, "Fallthrough");
        ui.colored_label(palette.warn, "Jump");
    });
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
    if canvas.dragged() {
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
            result = Some(GraphAction::Select(function.blocks[index].address));
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
            painter.add(egui::Shape::line(points.to_vec(), stroke));
            if zoom >= 0.16 {
                painter.arrow(end, to - end, stroke);
            }
        }
    }
    for (index, block) in function.blocks.iter().enumerate() {
        let rect = Rect::from_min_size(locate(state.layout.positions[index]), dimensions);
        if !canvas.rect.intersects(rect) {
            continue;
        }
        let highlight = selected_index == Some(index);
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
            let instructions = &function.instructions
                [block.first_instruction..block.first_instruction + block.instruction_count];
            let (last, head) = instructions.split_last().expect("nonempty decoded block");
            for (line, instruction) in head.iter().take(4).enumerate() {
                card.text(
                    rect.min + vec2(10.0, 28.0 + line as f32 * 15.0) * zoom,
                    Align2::LEFT_TOP,
                    &instruction.text,
                    FontId::monospace(12.0 * zoom),
                    palette.text,
                );
            }
            let line = head.len().min(4);
            if head.len() > 4 {
                card.text(
                    rect.min + vec2(10.0, 88.0) * zoom,
                    Align2::LEFT_TOP,
                    "…",
                    FontId::monospace(12.0 * zoom),
                    palette.muted,
                );
            }
            let last_line = line + usize::from(head.len() > 4);
            card.text(
                rect.min + vec2(10.0, 28.0 + last_line as f32 * 15.0) * zoom,
                Align2::LEFT_TOP,
                &last.text,
                FontId::monospace(12.0 * zoom),
                palette.text,
            );
        }
    }
    result
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
