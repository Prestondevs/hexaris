use eframe::egui::{
    self, Color32, ColorImage, Context, FontId, IconData, Stroke, TextureHandle, TextureOptions,
};

pub const ACCENT: Color32 = Color32::from_rgb(129, 227, 189);
pub const MUTED: Color32 = Color32::from_rgb(150, 167, 173);
pub const TEXT: Color32 = Color32::from_rgb(223, 233, 232);
pub const BG: Color32 = Color32::from_rgb(13, 19, 22);
pub const PANEL: Color32 = Color32::from_rgb(20, 27, 31);
pub const RAISED: Color32 = Color32::from_rgb(27, 37, 41);
pub const BORDER: Color32 = Color32::from_rgb(45, 61, 65);
pub const WARN: Color32 = Color32::from_rgb(240, 194, 126);
pub const MARK: &[u8] = include_bytes!("../../../assets/branding/hexaris.svg");

pub fn install(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.faint_bg_color = RAISED;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.inactive.bg_fill = RAISED;
    style.visuals.widgets.inactive.weak_bg_fill = RAISED;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 66, 64);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(43, 66, 64);
    style.visuals.selection.bg_fill = Color32::from_rgb(36, 70, 62);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 24.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(13.0));
    ctx.set_style(style);
}

pub fn frame(fill: Color32, side: i8, top: i8) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(side, top))
}

fn raster(svg: &[u8], size: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let scale = tiny_skia::Transform::from_scale(
        size as f32 / tree.size().width(),
        size as f32 / tree.size().height(),
    );
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    Some(pixmap.take())
}

pub fn texture(ctx: &Context, name: &str, svg: &[u8], size: u32) -> Option<TextureHandle> {
    let rgba = raster(svg, size)?;
    Some(ctx.load_texture(
        name,
        ColorImage::from_rgba_premultiplied([size as usize; 2], &rgba),
        TextureOptions::LINEAR,
    ))
}

pub fn window_icon() -> IconData {
    let Some(mut rgba) = raster(MARK, 64) else {
        return IconData::default();
    };
    for pixel in rgba.as_chunks_mut::<4>().0 {
        let alpha = u32::from(pixel[3]);
        if alpha > 0 && alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    IconData {
        rgba,
        width: 64,
        height: 64,
    }
}

pub fn tabler(ctx: &Context, name: &str, svg: &str) -> Option<TextureHandle> {
    texture(
        ctx,
        name,
        svg.replace("currentColor", "#DFE9E8").as_bytes(),
        48,
    )
}
