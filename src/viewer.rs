use egui::{Color32, Pos2, Rect, TextureHandle, Ui, Vec2};

#[derive(Clone, Copy, PartialEq)]
pub enum FitMode {
    FitWindow,
    FitWidth,
    FitHeight,
    ActualSize,
}

pub struct ViewerState {
    pub zoom: f32,
    pub offset: Vec2,
    pub fit_mode: FitMode,
    pub rotation: i32, // 0, 90, 180, 270
    pub flip_h: bool,
    pub flip_v: bool,
    needs_fit: bool,
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
            needs_fit: true,
        }
    }

    pub fn reset_for_new_image(&mut self) {
        self.zoom = 1.0;
        self.offset = Vec2::ZERO;
        self.needs_fit = true;
        self.rotation = 0;
        self.flip_h = false;
        self.flip_v = false;
    }

    pub fn zoom_percent(&self) -> f32 {
        self.zoom * 100.0
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
        match self.fit_mode {
            FitMode::FitWindow => {
                let scale_x = available.x / img.x;
                let scale_y = available.y / img.y;
                scale_x.min(scale_y).min(1.0)
            }
            FitMode::FitWidth => (available.x / img.x).min(1.0),
            FitMode::FitHeight => (available.y / img.y).min(1.0),
            FitMode::ActualSize => 1.0,
        }
    }

    pub fn handle_zoom_scroll(&mut self, delta: f32, cursor_pos: Pos2, canvas_center: Pos2) {
        let old_zoom = self.zoom;
        let factor = (1.0 + delta * 0.002).clamp(0.8, 1.25);
        self.zoom = (self.zoom * factor).clamp(0.01, 100.0);
        self.fit_mode = FitMode::ActualSize;

        let cursor_offset = Vec2::new(cursor_pos.x - canvas_center.x, cursor_pos.y - canvas_center.y) - self.offset;
        self.offset -= cursor_offset * (self.zoom / old_zoom - 1.0);
    }

    pub fn handle_pan(&mut self, delta: Vec2) {
        self.offset += delta;
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).clamp(0.01, 100.0);
        self.fit_mode = FitMode::ActualSize;
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).clamp(0.01, 100.0);
        self.fit_mode = FitMode::ActualSize;
    }

    pub fn set_fit_mode(&mut self, mode: FitMode) {
        self.fit_mode = mode;
        self.needs_fit = true;
        self.offset = Vec2::ZERO;
    }

    pub fn cycle_fit_mode(&mut self) {
        self.fit_mode = match self.fit_mode {
            FitMode::FitWindow => FitMode::FitWidth,
            FitMode::FitWidth => FitMode::FitHeight,
            FitMode::FitHeight => FitMode::ActualSize,
            FitMode::ActualSize => FitMode::FitWindow,
        };
        self.needs_fit = true;
        self.offset = Vec2::ZERO;
    }

    pub fn rotate_cw(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
        self.needs_fit = true;
        self.offset = Vec2::ZERO;
    }

    pub fn flip_horizontal(&mut self) {
        self.flip_h = !self.flip_h;
    }

    pub fn flip_vertical(&mut self) {
        self.flip_v = !self.flip_v;
    }

    pub fn paint(&mut self, ui: &mut Ui, texture: &TextureHandle, image_size: Vec2) {
        let available = ui.available_size();

        if self.needs_fit {
            self.zoom = self.calculate_fit_zoom(image_size, available);
            self.needs_fit = false;
        }

        let eff_size = self.effective_image_size(image_size);
        let scaled = eff_size * self.zoom;

        let canvas_rect = ui.available_rect_before_wrap();
        let center = canvas_rect.center();

        let min = Pos2::new(
            center.x - scaled.x * 0.5 + self.offset.x,
            center.y - scaled.y * 0.5 + self.offset.y,
        );
        let max = Pos2::new(min.x + scaled.x, min.y + scaled.y);
        let dest_rect = Rect::from_min_max(min, max);

        let mut uv_min = Pos2::new(0.0, 0.0);
        let mut uv_max = Pos2::new(1.0, 1.0);

        if self.flip_h {
            std::mem::swap(&mut uv_min.x, &mut uv_max.x);
        }
        if self.flip_v {
            std::mem::swap(&mut uv_min.y, &mut uv_max.y);
        }

        let uv = Rect::from_min_max(uv_min, uv_max);

        ui.painter()
            .image(texture.id(), dest_rect, uv, Color32::WHITE);

        // Handle interactions on the canvas area
        let response = ui.allocate_rect(canvas_rect, egui::Sense::click_and_drag());

        if response.dragged_by(egui::PointerButton::Primary) {
            self.handle_pan(response.drag_delta());
        }

        if let Some(pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.handle_zoom_scroll(scroll, pos, center);
            }
        }
    }
}
