use egui::{Color32, Pos2, Rect, Stroke, TextureHandle, Ui, Vec2, pos2};
use egui::epaint::{Mesh, Vertex};
use crate::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum FitMode {
    FitWindow,
    FitWidth,
    FitHeight,
    ActualSize,
}

#[derive(Clone, Copy)]
pub struct Selection {
    pub start: Pos2,
    pub end: Pos2,
}

impl Selection {
    pub fn rect(&self) -> Rect {
        Rect::from_two_pos(self.start, self.end)
    }

    pub fn is_significant(&self) -> bool {
        let r = self.rect();
        r.width() > 5.0 && r.height() > 5.0
    }
}

pub struct ViewerState {
    pub zoom: f32,
    pub offset: Vec2,
    pub fit_mode: FitMode,
    pub rotation: i32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub selection: Option<Selection>,
    drag_start: Option<Pos2>,
    needs_fit: bool,
    min_zoom: f32,
    last_scaled: Vec2,
    last_canvas: Vec2,
}

impl ViewerState {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            offset: Vec2::ZERO,
            fit_mode: FitMode::FitWindow,
            rotation: 0,
            flip_h: false,
            flip_v: false,
            selection: None,
            drag_start: None,
            needs_fit: true,
            min_zoom: 0.01,
            last_scaled: Vec2::ZERO,
            last_canvas: Vec2::ZERO,
        }
    }

    pub fn reset_for_new_image(&mut self) {
        self.zoom = 1.0;
        self.offset = Vec2::ZERO;
        self.fit_mode = FitMode::FitWindow;
        self.needs_fit = true;
        self.rotation = 0;
        self.flip_h = false;
        self.flip_v = false;
        self.selection = None;
        self.drag_start = None;
    }

    pub fn zoom_percent(&self) -> f32 {
        self.zoom * 100.0
    }

    pub fn effective_image_size_pub(&self, image_size: Vec2) -> Vec2 {
        self.effective_image_size(image_size)
    }

    fn effective_image_size(&self, image_size: Vec2) -> Vec2 {
        if self.rotation == 90 || self.rotation == 270 {
            Vec2::new(image_size.y, image_size.x)
        } else {
            image_size
        }
    }

    fn calculate_fit_zoom(&self, image_size: Vec2, available: Vec2) -> f32 {
        let img = self.effective_image_size(image_size);
        if img.x == 0.0 || img.y == 0.0 {
            return 1.0;
        }
        match self.fit_mode {
            FitMode::FitWindow => {
                let scale_x = available.x / img.x;
                let scale_y = available.y / img.y;
                scale_x.min(scale_y)
            }
            FitMode::FitWidth => available.x / img.x,
            FitMode::FitHeight => available.y / img.y,
            FitMode::ActualSize => 1.0,
        }
    }

    pub fn handle_zoom_scroll(&mut self, delta: f32, _cursor_pos: Pos2, _canvas_center: Pos2) {
        let factor = (1.0 + delta * 0.002).clamp(0.8, 1.25);
        self.zoom = (self.zoom * factor).clamp(0.01, 100.0);
        self.fit_mode = FitMode::ActualSize;
        self.selection = None;
        self.clamp_offset();
    }

    pub fn handle_pan(&mut self, delta: Vec2) {
        self.offset += delta;
        self.clamp_offset();
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).clamp(0.01, 100.0);
        self.fit_mode = FitMode::ActualSize;
        self.clamp_offset();
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).clamp(0.01, 100.0);
        self.fit_mode = FitMode::ActualSize;
        self.clamp_offset();
    }

    pub fn set_zoom(&mut self, level: f32) {
        self.zoom = level.clamp(0.01, 100.0);
        self.fit_mode = FitMode::ActualSize;
        self.offset = Vec2::ZERO;
        self.clamp_offset();
    }

    fn clamp_offset(&mut self) {
        let half_w = self.last_scaled.x * 0.5;
        let half_h = self.last_scaled.y * 0.5;
        let half_cw = self.last_canvas.x * 0.5;
        let half_ch = self.last_canvas.y * 0.5;

        let max_x = (half_w - half_cw).max(0.0);
        let max_y = (half_h - half_ch).max(0.0);

        self.offset.x = self.offset.x.clamp(-max_x, max_x);
        self.offset.y = self.offset.y.clamp(-max_y, max_y);
    }

    pub fn set_fit_mode(&mut self, mode: FitMode) {
        self.fit_mode = mode;
        self.needs_fit = true;
        self.offset = Vec2::ZERO;
        self.selection = None;
    }

    pub fn cycle_fit_mode(&mut self) {
        self.fit_mode = match self.fit_mode {
            FitMode::FitWindow => FitMode::ActualSize,
            _ => FitMode::FitWindow,
        };
        self.needs_fit = true;
        self.offset = Vec2::ZERO;
        self.selection = None;
    }

    pub fn rotate_cw(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
        self.needs_fit = true;
        self.offset = Vec2::ZERO;
    }

    pub fn rotate_ccw(&mut self) {
        self.rotation = (self.rotation + 270) % 360;
        self.needs_fit = true;
        self.offset = Vec2::ZERO;
    }

    pub fn flip_horizontal(&mut self) {
        self.flip_h = !self.flip_h;
    }

    pub fn flip_vertical(&mut self) {
        self.flip_v = !self.flip_v;
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.drag_start = None;
    }

    pub fn zoom_to_selection(&mut self, image_size: Vec2, canvas_rect: Rect) {
        let sel = match &self.selection {
            Some(s) if s.is_significant() => s.rect(),
            _ => return,
        };

        let eff_size = self.effective_image_size(image_size);
        let scaled = eff_size * self.zoom;
        let center = canvas_rect.center();
        let image_min = Pos2::new(
            center.x - scaled.x * 0.5 + self.offset.x,
            center.y - scaled.y * 0.5 + self.offset.y,
        );

        let sel_img_x = (sel.min.x - image_min.x) / self.zoom;
        let sel_img_y = (sel.min.y - image_min.y) / self.zoom;
        let sel_img_w = sel.width() / self.zoom;
        let sel_img_h = sel.height() / self.zoom;

        if sel_img_w <= 0.0 || sel_img_h <= 0.0 {
            return;
        }

        let new_zoom_x = canvas_rect.width() / sel_img_w;
        let new_zoom_y = canvas_rect.height() / sel_img_h;
        let new_zoom = new_zoom_x.min(new_zoom_y).clamp(0.01, 100.0);

        let sel_center_img_x = sel_img_x + sel_img_w * 0.5;
        let sel_center_img_y = sel_img_y + sel_img_h * 0.5;
        let img_center_x = eff_size.x * 0.5;
        let img_center_y = eff_size.y * 0.5;

        self.zoom = new_zoom;
        self.offset = Vec2::new(
            (img_center_x - sel_center_img_x) * new_zoom,
            (img_center_y - sel_center_img_y) * new_zoom,
        );
        self.fit_mode = FitMode::ActualSize;
        self.selection = None;
    }

    pub fn paint(&mut self, ui: &mut Ui, texture: &TextureHandle, image_size: Vec2) -> (Rect, bool) {
        let mut zoom_to_sel_requested = false;
        let available = ui.available_size();

        let eff_size = self.effective_image_size(image_size);

        if self.needs_fit {
            self.zoom = self.calculate_fit_zoom(image_size, available);
            self.offset = Vec2::ZERO;
            self.needs_fit = false;
        }

        let scaled = eff_size * self.zoom;
        self.last_scaled = scaled;
        self.last_canvas = available;

        let canvas_rect = ui.available_rect_before_wrap();
        let center = canvas_rect.center();

        let min = Pos2::new(
            center.x - scaled.x * 0.5 + self.offset.x,
            center.y - scaled.y * 0.5 + self.offset.y,
        );
        let max = Pos2::new(min.x + scaled.x, min.y + scaled.y);
        let dest_rect = Rect::from_min_max(min, max);

        // Build UV corners for rotation + flip via custom mesh
        let (mut uv_tl, mut uv_tr, mut uv_br, mut uv_bl) = match self.rotation {
            90  => (pos2(0.0, 1.0), pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0)),
            180 => (pos2(1.0, 1.0), pos2(0.0, 1.0), pos2(0.0, 0.0), pos2(1.0, 0.0)),
            270 => (pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0), pos2(0.0, 0.0)),
            _   => (pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)),
        };
        if self.flip_h {
            std::mem::swap(&mut uv_tl, &mut uv_tr);
            std::mem::swap(&mut uv_bl, &mut uv_br);
        }
        if self.flip_v {
            std::mem::swap(&mut uv_tl, &mut uv_bl);
            std::mem::swap(&mut uv_tr, &mut uv_br);
        }

        let tl = dest_rect.min;
        let tr = pos2(dest_rect.max.x, dest_rect.min.y);
        let br = dest_rect.max;
        let bl = pos2(dest_rect.min.x, dest_rect.max.y);
        let white = Color32::WHITE;

        let mut mesh = Mesh::with_texture(texture.id());
        mesh.vertices = vec![
            Vertex { pos: tl, uv: uv_tl, color: white },
            Vertex { pos: tr, uv: uv_tr, color: white },
            Vertex { pos: br, uv: uv_br, color: white },
            Vertex { pos: bl, uv: uv_bl, color: white },
        ];
        mesh.indices = vec![0, 1, 2, 0, 2, 3];
        ui.painter().add(egui::Shape::mesh(mesh));

        let response = ui.allocate_rect(canvas_rect, egui::Sense::click_and_drag());

        // Left-click-drag: draw selection rectangle
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if self.drag_start.is_none() {
                    self.drag_start = Some(pos);
                }
                if let Some(start) = self.drag_start {
                    self.selection = Some(Selection {
                        start,
                        end: pos,
                    });
                }
            }
        }

        if response.drag_stopped_by(egui::PointerButton::Primary) {
            self.drag_start = None;
            if let Some(sel) = &self.selection {
                if !sel.is_significant() {
                    self.selection = None;
                }
            }
        }

        // Right-click-drag: pan (when zoomed in)
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.handle_pan(response.drag_delta());
        }

        // Scroll wheel: zoom
        if let Some(pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.handle_zoom_scroll(scroll, pos, center);
            }
        }

        // Draw selection rectangle + magnifying glass
        if let Some(sel) = &self.selection {
            if sel.is_significant() {
                let sel_rect = sel.rect();
                ui.painter().rect_stroke(
                    sel_rect,
                    0.0,
                    Stroke::new(1.5, theme::ACCENT),
                    egui::StrokeKind::Outside,
                );
                // Rule-of-thirds grid inside selection
                let grid_stroke = Stroke::new(0.5, theme::ACCENT_RING);
                let third_w = sel_rect.width() / 3.0;
                let third_h = sel_rect.height() / 3.0;
                for i in 1..3 {
                    let x = sel_rect.min.x + third_w * i as f32;
                    ui.painter().line_segment(
                        [pos2(x, sel_rect.min.y), pos2(x, sel_rect.max.y)],
                        grid_stroke,
                    );
                    let y = sel_rect.min.y + third_h * i as f32;
                    ui.painter().line_segment(
                        [pos2(sel_rect.min.x, y), pos2(sel_rect.max.x, y)],
                        grid_stroke,
                    );
                }
                // Selection dimensions label
                let dim_text = format!("{:.0} x {:.0}", sel_rect.width(), sel_rect.height());
                ui.painter().text(
                    pos2(sel_rect.min.x, sel_rect.min.y - 4.0),
                    egui::Align2::LEFT_BOTTOM,
                    &dim_text,
                    egui::FontId::monospace(10.0),
                    theme::ACCENT,
                );
                // Dim area outside selection
                let dim = Color32::from_black_alpha(115);
                if sel_rect.min.y > canvas_rect.min.y {
                    ui.painter().rect_filled(
                        Rect::from_min_max(canvas_rect.min, Pos2::new(canvas_rect.max.x, sel_rect.min.y)),
                        0.0, dim,
                    );
                }
                if sel_rect.max.y < canvas_rect.max.y {
                    ui.painter().rect_filled(
                        Rect::from_min_max(Pos2::new(canvas_rect.min.x, sel_rect.max.y), canvas_rect.max),
                        0.0, dim,
                    );
                }
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(canvas_rect.min.x, sel_rect.min.y),
                        Pos2::new(sel_rect.min.x, sel_rect.max.y),
                    ),
                    0.0, dim,
                );
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(sel_rect.max.x, sel_rect.min.y),
                        Pos2::new(canvas_rect.max.x, sel_rect.max.y),
                    ),
                    0.0, dim,
                );

                // Magnifying glass when hovering inside selection
                if let Some(hover_pos) = response.hover_pos() {
                    if sel_rect.contains(hover_pos) && self.drag_start.is_none() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ZoomIn);

                        if response.clicked() {
                            zoom_to_sel_requested = true;
                        }
                    }
                }
            }
        }

        (canvas_rect, zoom_to_sel_requested)
    }
}
