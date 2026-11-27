use eframe::egui::{
    self, Color32, ColorImage, Context, FontId, IconData, Stroke, TextureHandle, TextureOptions,
};

const ARTWORK: &[u8] = include_bytes!("../../../assets/branding/starry-night.png");
pub const STORAGE_KEY: &str = "hexaris.theme";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Graphite,
    Slate,
    Ash,
    Foundry,
    HighContrast,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub warn: Color32,
    pub register: Color32,
    pub number: Color32,
    hover: Color32,
    selection: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    pub const ALL: [Self; 5] = [
        Self::Graphite,
        Self::Slate,
        Self::Ash,
        Self::Foundry,
        Self::HighContrast,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Graphite => "graphite",
            Self::Slate => "slate",
            Self::Ash => "ash",
            Self::Foundry => "foundry",
            Self::HighContrast => "high-contrast",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Graphite => "Graphite",
            Self::Slate => "Slate",
            Self::Ash => "Ash Light",
            Self::Foundry => "Foundry",
            Self::HighContrast => "High Contrast",
        }
    }

    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        storage
            .and_then(|store| store.get_string(STORAGE_KEY))
            .and_then(|id| Self::ALL.into_iter().find(|theme| theme.id() == id))
            .unwrap_or_default()
    }

    pub const fn palette(self) -> Palette {
        match self {
            Self::Graphite => Palette {
                bg: rgb(0x1C1D20),
                panel: rgb(0x24262A),
                raised: rgb(0x2D3035),
                border: rgb(0x42464D),
                text: rgb(0xEDF0F3),
                muted: rgb(0xAAB0B9),
                accent: rgb(0xA8C9F0),
                warn: rgb(0xEDC088),
                register: rgb(0xA6DEBD),
                number: rgb(0xDFC6F4),
                hover: rgb(0x393F47),
                selection: rgb(0x38495E),
            },
            Self::Slate => Palette {
                bg: rgb(0x1B212A),
                panel: rgb(0x222A35),
                raised: rgb(0x2B3543),
                border: rgb(0x455469),
                text: rgb(0xE7EEF8),
                muted: rgb(0xAFBCD0),
                accent: rgb(0xABC8FF),
                warn: rgb(0xF4C98D),
                register: rgb(0xA4DFCA),
                number: rgb(0xE4CBFF),
                hover: rgb(0x36455A),
                selection: rgb(0x354D71),
            },
            Self::Ash => Palette {
                bg: rgb(0xE7E9EC),
                panel: rgb(0xF0F1F3),
                raised: rgb(0xDDE1E6),
                border: rgb(0xAFB7C2),
                text: rgb(0x22272F),
                muted: rgb(0x525D6A),
                accent: rgb(0x215997),
                warn: rgb(0x8A4D12),
                register: rgb(0x185743),
                number: rgb(0x553779),
                hover: rgb(0xDEE5EE),
                selection: rgb(0xC5D9F1),
            },
            Self::Foundry => Palette {
                bg: rgb(0x221F1E),
                panel: rgb(0x2A2725),
                raised: rgb(0x35312E),
                border: rgb(0x544A42),
                text: rgb(0xF2EBE4),
                muted: rgb(0xBCB0A5),
                accent: rgb(0xF1BC94),
                warn: rgb(0xF4D08D),
                register: rgb(0xBAD8A7),
                number: rgb(0xD7C4F3),
                hover: rgb(0x403932),
                selection: rgb(0x533D32),
            },
            Self::HighContrast => Palette {
                bg: rgb(0x0D0D0D),
                panel: rgb(0x171717),
                raised: rgb(0x252525),
                border: rgb(0x777777),
                text: rgb(0xFFFFFF),
                muted: rgb(0xD1D1D1),
                accent: rgb(0xBBD9FF),
                warn: rgb(0xFFDD91),
                register: rgb(0xA6F7BC),
                number: rgb(0xE8C1FF),
                hover: rgb(0x383838),
                selection: rgb(0x274768),
            },
        }
    }

    pub fn previews(self, ui: &mut egui::Ui) -> bool {
        let palette = self.palette();
        let mut picked = false;
        for (label, color) in [
            ("Canvas", palette.bg),
            ("Panel", palette.panel),
            ("Accent", palette.accent),
            ("Text", palette.text),
        ] {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::click());
            ui.painter().rect(
                rect,
                2,
                color,
                Stroke::new(1.0_f32, palette.border),
                egui::StrokeKind::Inside,
            );
            picked |= response
                .on_hover_text(format!(
                    "{label} #{:02X}{:02X}{:02X}",
                    color.r(),
                    color.g(),
                    color.b()
                ))
                .clicked();
        }
        picked
    }
}

pub fn current(ctx: &Context) -> Palette {
    ctx.data(|data| data.get_temp::<Theme>(egui::Id::new(STORAGE_KEY)))
        .unwrap_or_default()
        .palette()
}

pub fn install(ctx: &Context, theme: Theme) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(STORAGE_KEY), theme));
    let base = if theme == Theme::Ash {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    };
    ctx.set_theme(base);
    let mut style = (*ctx.style()).clone();
    let palette = theme.palette();
    let mut visuals = base.default_visuals();
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.bg;
    visuals.override_text_color = Some(palette.text);
    visuals.faint_bg_color = palette.raised;
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warn;
    visuals.window_stroke.color = palette.border;
    visuals.widgets.noninteractive.bg_stroke.color = palette.border;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.fg_stroke.color = palette.text;
        widget.bg_stroke.color = palette.border;
    }
    for (widget, fill) in [
        (&mut visuals.widgets.inactive, palette.raised),
        (&mut visuals.widgets.hovered, palette.hover),
        (&mut visuals.widgets.active, palette.selection),
        (&mut visuals.widgets.open, palette.hover),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
    }
    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.accent);
    style.visuals = visuals;
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

pub fn artwork_texture(ctx: &Context) -> Option<TextureHandle> {
    let icon = eframe::icon_data::from_png_bytes(ARTWORK).ok()?;
    Some(ctx.load_texture(
        "hexaris-artwork",
        ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba),
        TextureOptions::LINEAR,
    ))
}

pub fn window_icon() -> IconData {
    eframe::icon_data::from_png_bytes(ARTWORK).unwrap_or_default()
}

pub fn tabler(ctx: &Context, name: &str, svg: &str) -> Option<TextureHandle> {
    let size = 48;
    let rgba = raster(svg.replace("currentColor", "#FFFFFF").as_bytes(), size)?;
    Some(ctx.load_texture(
        name,
        ColorImage::from_rgba_premultiplied([size as usize; 2], &rgba),
        TextureOptions::LINEAR,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_labels_are_readable_across_workbench_surfaces() {
        for theme in Theme::ALL {
            let palette = theme.palette();
            for surface in [palette.bg, palette.panel, palette.raised, palette.selection] {
                assert!(
                    contrast(palette.text, surface) >= 4.5,
                    "{} text",
                    theme.name()
                );
            }
            for ink in [palette.muted, palette.accent, palette.warn] {
                assert!(
                    contrast(ink, palette.panel) >= 4.5,
                    "{} panel text",
                    theme.name()
                );
            }
            for ink in [palette.register, palette.number] {
                for surface in [palette.panel, palette.hover, palette.selection] {
                    assert!(
                        contrast(ink, surface) >= 4.5,
                        "{} syntax text",
                        theme.name()
                    );
                }
            }
        }
    }

    fn contrast(first: Color32, second: Color32) -> f64 {
        let map = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        let luminance = |color: Color32| {
            0.2126 * map(color.r()) + 0.7152 * map(color.g()) + 0.0722 * map(color.b())
        };
        let (first, second) = (luminance(first), luminance(second));
        (first.max(second) + 0.05) / (first.min(second) + 0.05)
    }
}
