use egui::{Color32, Stroke, Style, Visuals};
use egui::epaint::CornerRadius;

// Cove design system colors
pub const ACCENT: Color32 = Color32::from_rgb(80, 230, 207);    // #50e6cf
pub const ACCENT_2: Color32 = Color32::from_rgb(61, 220, 151);  // #3ddc97
pub const BG: Color32 = Color32::from_rgb(11, 11, 16);          // #0b0b10
pub const SURFACE: Color32 = Color32::from_rgb(19, 19, 27);     // #13131b
pub const SURFACE_2: Color32 = Color32::from_rgb(24, 24, 34);   // #181822
pub const SURFACE_3: Color32 = Color32::from_rgb(31, 31, 43);   // #1f1f2b
pub const SURFACE_4: Color32 = Color32::from_rgb(38, 38, 53);   // #262635
pub const CANVAS_BG: Color32 = Color32::from_rgb(8, 8, 13);     // #08080d
pub const TEXT: Color32 = Color32::from_rgb(236, 236, 241);      // #ececf1
pub const TEXT_DIM: Color32 = Color32::from_rgb(154, 154, 174);  // #9a9aae
pub const TEXT_FAINT: Color32 = Color32::from_rgb(107, 107, 128);// #6b6b80
pub const DANGER: Color32 = Color32::from_rgb(255, 107, 107);   // #ff6b6b
pub const BORDER: Color32 = Color32::from_rgb(20, 20, 28);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(30, 30, 40);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(14, 30, 28);
pub const ACCENT_RING: Color32 = Color32::from_rgb(28, 80, 72);

pub fn apply_cove_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    let r6 = CornerRadius::same(6);

    visuals.panel_fill = SURFACE;
    visuals.window_fill = BG;
    visuals.extreme_bg_color = CANVAS_BG;
    visuals.faint_bg_color = SURFACE_2;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;

    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.corner_radius = r6;

    visuals.widgets.inactive.bg_fill = SURFACE_2;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_2;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.corner_radius = r6;

    visuals.widgets.hovered.bg_fill = SURFACE_3;
    visuals.widgets.hovered.weak_bg_fill = SURFACE_3;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.hovered.corner_radius = r6;

    visuals.widgets.active.bg_fill = SURFACE_4;
    visuals.widgets.active.weak_bg_fill = SURFACE_4;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_RING);
    visuals.widgets.active.corner_radius = r6;

    visuals.widgets.open.bg_fill = SURFACE_3;
    visuals.widgets.open.weak_bg_fill = SURFACE_3;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.open.corner_radius = r6;

    visuals.window_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(180),
    };
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 16],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(200),
    };

    let mut style = Style::default();
    style.visuals = visuals;
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);

    ctx.set_style(style);
}
