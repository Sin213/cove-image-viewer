use crate::browser::{Browser, SortMode};
use crate::decoder::{self, DecodedImage};
use crate::theme;
use crate::viewer::{FitMode, ViewerState};
use egui::{TextureHandle, Vec2};
use std::path::{Path, PathBuf};

pub struct CoveApp {
    browser: Browser,
    viewer: ViewerState,
    current_texture: Option<TextureHandle>,
    current_image_size: Vec2,
    current_path: Option<PathBuf>,
    current_format: String,
    current_file_size: u64,
    error: Option<(PathBuf, String)>,
    pending_image: Option<DecodedImage>,
    fullscreen: bool,
    show_info: bool,
    slideshow_active: bool,
    slideshow_interval: f32,
    slideshow_timer: f32,
    canvas_rect: egui::Rect,
    current_pixels: Option<egui::ColorImage>,
    lock_zoom: bool,
    always_on_top: bool,
    show_about: bool,
    show_image_info: bool,
    confirm_delete: Option<PathBuf>,
    pending_crop: bool,
    pending_undo: bool,
    undo_stack: Vec<(egui::ColorImage, Vec2)>,
    icon_texture: Option<TextureHandle>,
    tb_press_pos: Option<egui::Pos2>,
    anim_frames: Vec<(egui::ColorImage, u32)>,
    anim_textures: Vec<TextureHandle>,
    anim_index: usize,
    anim_timer: f32,
}

impl CoveApp {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut app = Self {
            browser: Browser::new(),
            viewer: ViewerState::new(),
            current_texture: None,
            current_image_size: Vec2::ZERO,
            current_path: None,
            current_format: String::new(),
            current_file_size: 0,
            error: None,
            pending_image: None,
            fullscreen: false,
            show_info: false,
            slideshow_active: false,
            slideshow_interval: 5.0,
            slideshow_timer: 0.0,
            canvas_rect: egui::Rect::NOTHING,
            current_pixels: None,
            lock_zoom: false,
            always_on_top: false,
            show_about: false,
            show_image_info: false,
            confirm_delete: None,
            pending_crop: false,
            pending_undo: false,
            undo_stack: Vec::new(),
            icon_texture: None,
            tb_press_pos: None,
            anim_frames: Vec::new(),
            anim_textures: Vec::new(),
            anim_index: 0,
            anim_timer: 0.0,
        };

        if let Some(p) = path {
            let canonical = std::fs::canonicalize(&p).unwrap_or(p);
            app.browser.open_path(&canonical);
            app.load_current();
        }

        app
    }

    fn load_current(&mut self) {
        if let Some(path) = self.browser.current() {
            let path = path.to_path_buf();
            self.load_image(&path);
        }
    }

    fn load_image(&mut self, path: &Path) {
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        self.anim_frames.clear();
        self.anim_textures.clear();
        self.anim_index = 0;
        self.anim_timer = 0.0;

        if let Some(frames) = decoder::load_animated(path) {
            let first = &frames[0];
            let w = first.pixels.width() as u32;
            let h = first.pixels.height() as u32;
            self.anim_frames = frames.into_iter().map(|f| (f.pixels, f.delay_ms)).collect();
            self.current_path = Some(path.to_path_buf());
            self.current_image_size = Vec2::new(w as f32, h as f32);
            self.current_format = path.extension().and_then(|e| e.to_str()).map(|e| e.to_uppercase()).unwrap_or_else(|| "GIF".into());
            self.current_file_size = file_size;
            self.error = None;
            self.pending_image = None;
            self.undo_stack.clear();
            self.viewer.reset_for_new_image();
            return;
        }

        match decoder::load_image(path) {
            Ok(decoded) => {
                let locked_zoom = if self.lock_zoom {
                    Some((self.viewer.zoom, self.viewer.fit_mode))
                } else {
                    None
                };
                self.viewer.reset_for_new_image();
                if let Some((z, fm)) = locked_zoom {
                    self.viewer.zoom = z;
                    self.viewer.fit_mode = fm;
                }
                self.current_path = Some(path.to_path_buf());
                self.current_image_size = Vec2::new(
                    decoded.original_width as f32,
                    decoded.original_height as f32,
                );
                self.current_format = decoded.format_name.clone();
                self.current_file_size = file_size;
                self.error = None;
                self.pending_image = Some(decoded);
                self.undo_stack.clear();
            }
            Err(e) => {
                self.error = Some((path.to_path_buf(), e));
                self.current_texture = None;
                self.current_path = Some(path.to_path_buf());
            }
        }
    }

    fn copy_to_clipboard(&self) {
        if let Some(path) = &self.current_path {
            if let Ok(img) = image::open(path) {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                let img_data = arboard::ImageData {
                    width: w,
                    height: h,
                    bytes: rgba.into_raw().into(),
                };
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_image(img_data);
                }
            }
        }
    }

    fn set_as_wallpaper(&self) {
        if let Some(path) = &self.current_path {
            if let Some(path_str) = path.to_str() {
                let _ = wallpaper::set_from_path(path_str);
            }
        }
    }

    fn crop_to_selection(&mut self, ctx: &egui::Context) {
        let sel = match &self.viewer.selection {
            Some(s) if s.is_significant() => s.rect(),
            _ => return,
        };
        let pixels = match &self.current_pixels {
            Some(p) => p,
            None => return,
        };

        let orig_w = pixels.width();
        let orig_h = pixels.height();
        let eff_size = self.viewer.effective_image_size_pub(self.current_image_size);
        let scaled = eff_size * self.viewer.zoom;
        let center = self.canvas_rect.center();
        let img_min = egui::Pos2::new(
            center.x - scaled.x * 0.5 + self.viewer.offset.x,
            center.y - scaled.y * 0.5 + self.viewer.offset.y,
        );

        let ex1 = ((sel.min.x - img_min.x) / self.viewer.zoom).max(0.0) as usize;
        let ey1 = ((sel.min.y - img_min.y) / self.viewer.zoom).max(0.0) as usize;
        let ex2 = ((sel.max.x - img_min.x) / self.viewer.zoom).ceil() as usize;
        let ey2 = ((sel.max.y - img_min.y) / self.viewer.zoom).ceil() as usize;

        self.undo_stack.push((pixels.clone(), self.current_image_size));

        let transformed = apply_transform(pixels, self.viewer.rotation, self.viewer.flip_h, self.viewer.flip_v);
        let tw = transformed.width();
        let th = transformed.height();

        let x1 = ex1.min(tw);
        let y1 = ey1.min(th);
        let x2 = ex2.min(tw);
        let y2 = ey2.min(th);
        let cw = x2.saturating_sub(x1);
        let ch = y2.saturating_sub(y1);

        if cw == 0 || ch == 0 {
            return;
        }

        let mut cropped_pixels = Vec::with_capacity(cw * ch);
        for y in y1..y2 {
            for x in x1..x2 {
                cropped_pixels.push(transformed.pixels[y * tw + x]);
            }
        }

        let cropped = egui::ColorImage {
            size: [cw, ch],
            pixels: cropped_pixels,
        };

        let texture = ctx.load_texture("current_image", cropped.clone(), egui::TextureOptions::LINEAR);
        self.current_texture = Some(texture);
        self.current_pixels = Some(cropped);
        self.current_image_size = Vec2::new(cw as f32, ch as f32);
        self.viewer.rotation = 0;
        self.viewer.flip_h = false;
        self.viewer.flip_v = false;
        self.viewer.selection = None;
        self.viewer.set_fit_mode(FitMode::FitWindow);
    }

    fn undo_crop(&mut self, ctx: &egui::Context) {
        if let Some((pixels, size)) = self.undo_stack.pop() {
            let texture = ctx.load_texture("current_image", pixels.clone(), egui::TextureOptions::LINEAR);
            self.current_texture = Some(texture);
            self.current_pixels = Some(pixels);
            self.current_image_size = size;
            self.viewer.rotation = 0;
            self.viewer.flip_h = false;
            self.viewer.flip_v = false;
            self.viewer.selection = None;
            self.viewer.set_fit_mode(FitMode::FitWindow);
        }
    }

    fn delete_current_file(&mut self) {
        if let Some(path) = &self.current_path {
            let path = path.clone();
            if std::fs::remove_file(&path).is_ok() {
                self.browser.remove_current();
                self.load_current();
            }
        }
    }

    fn navigate(&mut self, forward: bool) {
        let changed = if forward {
            self.browser.next()
        } else {
            self.browser.prev()
        };
        if changed {
            self.load_current();
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open Image")
            .add_filter(
                "Images",
                &[
                    "jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "webp", "svg", "ico",
                    "psd", "jxl", "qoi", "exr", "hdr", "tga", "dds", "pnm", "ppm", "pgm",
                    "pbm", "ff", "cr2", "nef", "arw", "dng", "heic", "heif", "avif",
                    "jp2", "j2k", "pcx", "xbm", "xpm", "sgi",
                ],
            )
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.browser.open_path(&path);
            self.load_current();
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.first() {
            if let Some(path) = &file.path {
                self.browser.open_path(path);
                self.load_current();
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let mut nav_forward = false;
        let mut nav_back = false;
        let mut go_home = false;
        let mut go_end = false;
        let mut toggle_fullscreen = false;
        let mut open_dialog = false;
        let mut zoom_out = false;
        let mut crop_to_selection = false;
        let mut copy_to_clip = false;
        let mut delete_file = false;
        let mut undo = false;
        let mut text_actions: Vec<String> = Vec::new();

        ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowRight) {
                nav_forward = true;
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                nav_back = true;
            }
            if i.key_pressed(egui::Key::Home) {
                go_home = true;
            }
            if i.key_pressed(egui::Key::End) {
                go_end = true;
            }
            if i.key_pressed(egui::Key::F11) {
                toggle_fullscreen = true;
            }
            if i.key_pressed(egui::Key::Escape) {
                if self.viewer.selection.is_some() {
                    self.viewer.clear_selection();
                } else if self.slideshow_active {
                    self.slideshow_active = false;
                } else if self.fullscreen {
                    toggle_fullscreen = true;
                }
            }
            if i.key_pressed(egui::Key::Minus) {
                zoom_out = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::O) {
                open_dialog = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::Y) {
                crop_to_selection = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::C) {
                copy_to_clip = true;
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::Z) {
                undo = true;
            }
            if i.key_pressed(egui::Key::Delete) {
                delete_file = true;
            }

            for event in &i.events {
                if let egui::Event::Text(t) = event {
                    text_actions.push(t.clone());
                }
            }
        });

        if nav_forward {
            self.navigate(true);
        }
        if nav_back {
            self.navigate(false);
        }
        if go_home && self.browser.first() {
            self.load_current();
        }
        if go_end && self.browser.last() {
            self.load_current();
        }
        if toggle_fullscreen {
            self.fullscreen = !self.fullscreen;
        }
        if zoom_out {
            self.viewer.zoom_out();
        }
        if open_dialog {
            self.open_file_dialog();
        }
        if crop_to_selection {
            self.crop_to_selection(ctx);
        }
        if undo {
            self.undo_crop(ctx);
        }
        if copy_to_clip {
            self.copy_to_clipboard();
        }
        if delete_file {
            if let Some(p) = &self.current_path {
                self.confirm_delete = Some(p.clone());
            }
        }

        for t in &text_actions {
            match t.as_str() {
                "+" | "=" => self.viewer.zoom_in(),
                "0" => self.viewer.set_fit_mode(FitMode::FitWindow),
                "1" => self.viewer.set_fit_mode(FitMode::ActualSize),
                "f" => self.viewer.cycle_fit_mode(),
                "i" => self.show_info = !self.show_info,
                "r" => self.viewer.rotate_cw(),
                "l" => self.viewer.rotate_ccw(),
                "h" => self.viewer.flip_horizontal(),
                "v" => self.viewer.flip_vertical(),
                "s" => {
                    self.slideshow_active = !self.slideshow_active;
                    self.slideshow_timer = 0.0;
                    if self.slideshow_active {
                        self.fullscreen = true;
                    }
                }
                _ => {}
            }
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    fn update_slideshow(&mut self, ctx: &egui::Context) {
        if !self.slideshow_active {
            return;
        }
        self.slideshow_timer += ctx.input(|i| i.unstable_dt);
        if self.slideshow_timer >= self.slideshow_interval {
            self.slideshow_timer = 0.0;
            self.navigate(true);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    fn draw_titlebar(&mut self, ctx: &egui::Context) {
        if self.fullscreen {
            return;
        }

        egui::TopBottomPanel::top("titlebar")
            .exact_height(40.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin { left: 12, right: 8, top: 0, bottom: 0 })
                    .stroke(egui::Stroke::new(1.0, theme::BORDER)),
            )
            .show(ctx, |ui| {
                let panel_rect = ui.available_rect_before_wrap();
                let mut btn_rects: Vec<egui::Rect> = Vec::new();

                ui.horizontal_centered(|ui| {
                    if let Some(tex) = &self.icon_texture {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(24.0, 24.0)));
                    }

                    ui.add_space(8.0);

                    let title_rect = ui.available_rect_before_wrap();
                    let title_center = title_rect.center();
                    ui.painter().text(
                        egui::pos2(title_center.x, title_center.y),
                        egui::Align2::CENTER_CENTER,
                        "Cove Image Viewer v1.0.0",
                        egui::FontId::proportional(12.0),
                        theme::TEXT_DIM,
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;

                        let close_r = ui.add(egui::Button::new(
                            egui::RichText::new("x").size(13.0).color(theme::TEXT_FAINT),
                        ).min_size(egui::vec2(36.0, 28.0)));
                        if close_r.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        btn_rects.push(close_r.rect);

                        let max_r = ui.add(egui::Button::new(
                            egui::RichText::new("\u{25A1}").size(12.0).color(theme::TEXT_FAINT),
                        ).min_size(egui::vec2(36.0, 28.0)));
                        if max_r.clicked() {
                            let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
                        }
                        btn_rects.push(max_r.rect);

                        let min_r = ui.add(egui::Button::new(
                            egui::RichText::new("\u{2013}").size(13.0).color(theme::TEXT_FAINT),
                        ).min_size(egui::vec2(36.0, 28.0)));
                        if min_r.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        btn_rects.push(min_r.rect);
                    });
                });

                let (pressed, released, dbl, held) = ctx.input(|i| (
                    i.pointer.button_pressed(egui::PointerButton::Primary),
                    i.pointer.button_released(egui::PointerButton::Primary),
                    i.pointer.button_double_clicked(egui::PointerButton::Primary),
                    i.pointer.button_down(egui::PointerButton::Primary),
                ));

                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    let on_panel = panel_rect.contains(pos);
                    let on_button = btn_rects.iter().any(|r| r.contains(pos));

                    if dbl && on_panel && !on_button {
                        let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
                        self.tb_press_pos = None;
                    } else if pressed && on_panel && !on_button {
                        self.tb_press_pos = Some(pos);
                    } else if held && self.tb_press_pos.is_some() {
                        let origin = self.tb_press_pos.unwrap();
                        let dist = (pos - origin).length();
                        if dist > 4.0 {
                            self.tb_press_pos = None;
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }
                }

                if released || !held {
                    self.tb_press_pos = None;
                }
            });
    }

    fn draw_menu_bar(&mut self, ctx: &egui::Context) {
        if self.fullscreen {
            return;
        }

        egui::TopBottomPanel::top("menu_bar")
            .frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(6, 2))
                .stroke(egui::Stroke::new(1.0, theme::BORDER)))
            .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...          Ctrl+O").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Slideshow             S").clicked() {
                        self.slideshow_active = !self.slideshow_active;
                        self.slideshow_timer = 0.0;
                        if self.slideshow_active { self.fullscreen = true; }
                        ui.close_menu();
                    }
                    ui.separator();
                    let del_enabled = self.current_path.is_some();
                    if ui.add_enabled(del_enabled, egui::Button::new("Delete File         Del")).clicked() {
                        if let Some(p) = &self.current_path {
                            self.confirm_delete = Some(p.clone());
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit                Esc").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    let can_undo = !self.undo_stack.is_empty();
                    if ui.add_enabled(can_undo, egui::Button::new("Undo              Ctrl+Z")).clicked() {
                        self.pending_undo = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Copy              Ctrl+C").clicked() {
                        self.copy_to_clipboard();
                        ui.close_menu();
                    }
                    ui.separator();
                    let has_sel = self.viewer.selection.as_ref().map(|s| s.is_significant()).unwrap_or(false);
                    if ui.add_enabled(has_sel, egui::Button::new("Crop Selection   Ctrl+Y")).clicked() {
                        // Can't call crop_to_selection here (needs ctx), so set flag
                        self.pending_crop = true;
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_sel, egui::Button::new("Zoom to Selection")).clicked() {
                        self.viewer.zoom_to_selection(self.current_image_size, self.canvas_rect);
                        ui.close_menu();
                    }
                    if ui.button("Clear Selection     Esc").clicked() {
                        self.viewer.clear_selection();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Set as Wallpaper").clicked() {
                        self.set_as_wallpaper();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Image", |ui| {
                    if ui.button("Information           I").clicked() {
                        self.show_image_info = !self.show_image_info;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Rotate Left           L").clicked() {
                        self.viewer.rotate_ccw();
                        ui.close_menu();
                    }
                    if ui.button("Rotate Right          R").clicked() {
                        self.viewer.rotate_cw();
                        ui.close_menu();
                    }
                    if ui.button("Flip Horizontal       H").clicked() {
                        self.viewer.flip_horizontal();
                        ui.close_menu();
                    }
                    if ui.button("Flip Vertical         V").clicked() {
                        self.viewer.flip_vertical();
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Full Screen         F11").clicked() {
                        self.fullscreen = !self.fullscreen;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Zoom In               +").clicked() {
                        self.viewer.zoom_in();
                        ui.close_menu();
                    }
                    if ui.button("Zoom Out              -").clicked() {
                        self.viewer.zoom_out();
                        ui.close_menu();
                    }
                    if ui.button("Fit to Window         0").clicked() {
                        self.viewer.set_fit_mode(FitMode::FitWindow);
                        ui.close_menu();
                    }
                    if ui.button("Original Size         1").clicked() {
                        self.viewer.set_fit_mode(FitMode::ActualSize);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.checkbox(&mut self.lock_zoom, "Lock Zoom").changed() {
                        ui.close_menu();
                    }
                    if ui.checkbox(&mut self.always_on_top, "Always on Top").changed() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                            if self.always_on_top {
                                egui::WindowLevel::AlwaysOnTop
                            } else {
                                egui::WindowLevel::Normal
                            },
                        ));
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Sort Files", |ui| {
                        let current = self.browser.sort_mode;
                        if ui.selectable_label(current == SortMode::Name, "By Name").clicked() {
                            self.browser.sort_by(SortMode::Name);
                            ui.close_menu();
                        }
                        if ui.selectable_label(current == SortMode::DateModified, "By Date Modified").clicked() {
                            self.browser.sort_by(SortMode::DateModified);
                            ui.close_menu();
                        }
                        if ui.selectable_label(current == SortMode::Size, "By Size").clicked() {
                            self.browser.sort_by(SortMode::Size);
                            ui.close_menu();
                        }
                        if ui.selectable_label(current == SortMode::Extension, "By Extension").clicked() {
                            self.browser.sort_by(SortMode::Extension);
                            ui.close_menu();
                        }
                    });
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About Cove").clicked() {
                        self.show_about = !self.show_about;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        if self.fullscreen {
            return;
        }

        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(12, 8))
                .stroke(egui::Stroke::new(1.0, theme::BORDER)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().button_padding = egui::vec2(9.0, 5.0);
                ui.spacing_mut().item_spacing.x = 6.0;

                if ui.button("\u{1F4C2} Open").clicked() {
                    self.open_file_dialog();
                }

                ui.separator();

                if ui.button("\u{25C0}").on_hover_text("Previous (Left)").clicked() {
                    self.navigate(false);
                }
                if ui.button("\u{25B6}").on_hover_text("Next (Right)").clicked() {
                    self.navigate(true);
                }

                ui.separator();

                if ui.button("\u{1F50D}+").on_hover_text("Zoom in (+)").clicked() {
                    self.viewer.zoom_in();
                }

                let zoom_text = format!("{:.0}% \u{25BE}", self.viewer.zoom_percent());
                egui::ComboBox::from_id_salt("zoom_combo")
                    .selected_text(&zoom_text)
                    .width(70.0)
                    .show_ui(ui, |ui| {
                        for &pct in &[10, 25, 33, 50, 66, 75, 100, 125, 150, 200, 300, 500] {
                            if ui.selectable_label(false, format!("{pct}%")).clicked() {
                                self.viewer.set_zoom(pct as f32 / 100.0);
                            }
                        }
                        ui.separator();
                        if ui.selectable_label(false, "Fit to window").clicked() {
                            self.viewer.set_fit_mode(FitMode::FitWindow);
                        }
                        if ui.selectable_label(false, "Fit width").clicked() {
                            self.viewer.set_fit_mode(FitMode::FitWidth);
                        }
                        if ui.selectable_label(false, "Fit height").clicked() {
                            self.viewer.set_fit_mode(FitMode::FitHeight);
                        }
                    });

                if ui.button("\u{1F50D}\u{2212}").on_hover_text("Zoom out (-)").clicked() {
                    self.viewer.zoom_out();
                }

                if ui.button("\u{2922} Fit").on_hover_text("Fit to window (0)").clicked() {
                    self.viewer.set_fit_mode(FitMode::FitWindow);
                }
                if ui.button("\u{1D7D9} 1:1").on_hover_text("Actual size (1)").clicked() {
                    self.viewer.set_fit_mode(FitMode::ActualSize);
                }

                ui.separator();

                let has_selection = self
                    .viewer
                    .selection
                    .as_ref()
                    .map(|s| s.is_significant())
                    .unwrap_or(false);

                if ui
                    .add_enabled(has_selection, egui::Button::new("\u{2702} Crop"))
                    .on_hover_text("Crop to selection (Ctrl+Y)")
                    .clicked()
                {
                    self.pending_crop = true;
                }
                if ui
                    .add_enabled(has_selection, egui::Button::new("\u{1F50D}\u{2610}"))
                    .on_hover_text("Zoom to selection")
                    .clicked()
                {
                    self.viewer
                        .zoom_to_selection(self.current_image_size, self.canvas_rect);
                }

                ui.separator();

                if ui.button("\u{21BB}").on_hover_text("Rotate right (R)").clicked() {
                    self.viewer.rotate_cw();
                }
                if ui.button("\u{2B0C}").on_hover_text("Flip horizontal (H)").clicked() {
                    self.viewer.flip_horizontal();
                }
                if ui.button("\u{2B0D}").on_hover_text("Flip vertical (V)").clicked() {
                    self.viewer.flip_vertical();
                }

                ui.separator();

                if ui.button("\u{26F6}").on_hover_text("Fullscreen (F11)").clicked() {
                    self.fullscreen = !self.fullscreen;
                }

                let slideshow_label = if self.slideshow_active {
                    "\u{25A0} Stop"
                } else {
                    "\u{25B6} Slideshow"
                };
                if ui.button(slideshow_label).on_hover_text("Slideshow (S)").clicked() {
                    self.slideshow_active = !self.slideshow_active;
                    self.slideshow_timer = 0.0;
                    if self.slideshow_active {
                        self.fullscreen = true;
                    }
                }
            });
        });
    }

    fn draw_status_bar(&self, ctx: &egui::Context) {
        if self.fullscreen && !self.show_info {
            return;
        }

        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(12, 4))
                .stroke(egui::Stroke::new(1.0, theme::BORDER)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let dot_rect = ui.allocate_space(egui::vec2(8.0, 8.0));
                ui.painter().circle_filled(
                    dot_rect.1.center(),
                    3.0,
                    theme::ACCENT_2,
                );

                ui.add_space(4.0);

                if self.current_texture.is_none() && self.error.is_none() {
                    ui.colored_label(theme::TEXT_DIM, "No image loaded \u{2014} Ctrl+O to open, or drag & drop");
                    return;
                }

                if let Some((path, msg)) = &self.error {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    ui.colored_label(theme::DANGER, format!("{name}: {msg}"));
                    ui.separator();
                    ui.colored_label(theme::TEXT_DIM, self.browser.position_label());
                    return;
                }

                if let Some(path) = &self.current_path {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    ui.colored_label(theme::TEXT, name);
                    ui.separator();
                    ui.colored_label(theme::TEXT_DIM, format!(
                        "{} \u{00D7} {}",
                        self.current_image_size.x as u32,
                        self.current_image_size.y as u32
                    ));
                    ui.separator();
                    ui.colored_label(theme::ACCENT, &self.current_format);
                    ui.separator();
                    ui.colored_label(theme::TEXT_DIM, decoder::format_file_size(self.current_file_size));
                    ui.separator();
                    ui.colored_label(theme::TEXT_DIM, format!("zoom {:.0}%", self.viewer.zoom_percent()));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(theme::TEXT_DIM, self.browser.position_label());

                        if self.slideshow_active {
                            ui.separator();
                            ui.colored_label(theme::ACCENT_2, "\u{25B6} Slideshow");
                        }

                        if let Some(sel) = &self.viewer.selection {
                            if sel.is_significant() {
                                let r = sel.rect();
                                ui.separator();
                                ui.colored_label(theme::ACCENT, format!(
                                    "Sel: {:.0}\u{00D7}{:.0}",
                                    r.width(),
                                    r.height()
                                ));
                            }
                        }
                    });
                }
            });
        });
    }

    fn draw_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::CANVAS_BG))
            .show(ctx, |ui| {
                if let Some(ref texture) = self.current_texture {
                    let tex = texture.clone();
                    let size = self.current_image_size;
                    let (rect, zoom_sel) = self.viewer.paint(ui, &tex, size);
                    self.canvas_rect = rect;
                    if zoom_sel {
                        self.viewer.zoom_to_selection(self.current_image_size, self.canvas_rect);
                    }
                } else if let Some((_, msg)) = &self.error {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(theme::DANGER, format!("Failed to load: {msg}"));
                    });
                } else {
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(ui.available_rect_before_wrap()), |ui| {
                        ui.vertical_centered(|ui| {
                            let avail = ui.available_height();
                            ui.add_space(avail * 0.3);
                            ui.label(
                                egui::RichText::new("Drop an image to open")
                                    .size(17.0)
                                    .color(theme::TEXT),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("Cove opens virtually anything \u{2014} drag a file in, or browse your disk.")
                                    .size(12.5)
                                    .color(theme::TEXT_DIM),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new("\u{2014} OR \u{2014}")
                                    .size(10.0)
                                    .color(theme::TEXT_FAINT),
                            );
                            ui.add_space(10.0);
                            if ui.button("\u{1F4C2} Open image\u{2026}").clicked() {
                                self.open_file_dialog();
                            }
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new("JPG \u{00B7} PNG \u{00B7} GIF \u{00B7} WebP \u{00B7} AVIF \u{00B7} HEIC \u{00B7} SVG \u{00B7} PSD \u{00B7} TIFF \u{00B7} RAW \u{00B7} JXL \u{00B7} JP2 \u{00B7} ICO \u{00B7} BMP \u{00B7} QOI \u{00B7} EXR \u{00B7} and 30 more")
                                    .size(10.0)
                                    .color(theme::TEXT_FAINT),
                            );
                        });
                    });
                }
            });
    }

    fn draw_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.confirm_delete.clone() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
            egui::Window::new("Delete File")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!("Delete \"{name}\"?"));
                    ui.label("This cannot be undone.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.confirm_delete = None;
                            self.delete_current_file();
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_delete = None;
                        }
                    });
                });
        }

        if self.show_image_info {
            if let Some(path) = &self.current_path {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
                let dir = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
                let w = self.current_image_size.x as u32;
                let h = self.current_image_size.y as u32;
                let fmt = self.current_format.clone();
                let size = decoder::format_file_size(self.current_file_size);
                let modified = std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| {
                        let dur = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                        let secs = dur.as_secs();
                        Some(format_timestamp(secs))
                    })
                    .unwrap_or_else(|| "Unknown".to_string());

                let mut open = self.show_image_info;
                egui::Window::new("Image Information")
                    .open(&mut open)
                    .resizable(false)
                    .default_width(350.0)
                    .show(ctx, |ui| {
                        egui::Grid::new("info_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                            ui.label("File:");
                            ui.label(&name);
                            ui.end_row();
                            ui.label("Directory:");
                            ui.label(&dir);
                            ui.end_row();
                            ui.label("Dimensions:");
                            ui.label(format!("{w} x {h} pixels"));
                            ui.end_row();
                            ui.label("Format:");
                            ui.label(&fmt);
                            ui.end_row();
                            ui.label("File Size:");
                            ui.label(&size);
                            ui.end_row();
                            ui.label("Modified:");
                            ui.label(&modified);
                            ui.end_row();
                            ui.label("Zoom:");
                            ui.label(format!("{:.1}%", self.viewer.zoom_percent()));
                            ui.end_row();
                            ui.label("Position:");
                            ui.label(self.browser.position_label());
                            ui.end_row();
                        });
                    });
                self.show_image_info = open;
            }
        }

        if self.show_about {
            let screen = ctx.screen_rect();
            let backdrop_layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("about_backdrop"));
            let painter = ctx.layer_painter(backdrop_layer);
            painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));

            let card_width = 380.0;
            egui::Area::new(egui::Id::new("about_area"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(theme::BG)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_STRONG))
                        .corner_radius(14.0)
                        .inner_margin(egui::Margin::same(0))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 20],
                            blur: 60,
                            spread: 0,
                            color: egui::Color32::from_black_alpha(200),
                        })
                        .show(ui, |ui| {
                            ui.set_width(card_width);

                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                if let Some(tex) = &self.icon_texture {
                                    ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(22.0, 22.0)));
                                }
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("About").size(13.0).color(theme::TEXT).strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(6.0);
                                    if ui.add(egui::Button::new(
                                        egui::RichText::new("x").size(15.0).color(theme::TEXT_FAINT),
                                    ).min_size(egui::vec2(32.0, 28.0))).clicked() {
                                        self.show_about = false;
                                    }
                                });
                            });
                            ui.add_space(2.0);

                            ui.add_space(2.0);
                            let sep_rect = egui::Rect::from_min_size(
                                egui::pos2(ui.min_rect().left(), ui.cursor().top()),
                                egui::vec2(card_width, 1.0),
                            );
                            ui.painter().rect_filled(sep_rect, 0.0, theme::BORDER);
                            ui.add_space(3.0);

                            ui.vertical_centered(|ui| {
                                ui.set_max_width(card_width - 40.0);
                                ui.add_space(12.0);

                                if let Some(tex) = &self.icon_texture {
                                    ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(72.0, 72.0)));
                                }

                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("Cove Image Viewer").size(19.0).strong().color(theme::TEXT));
                                ui.add_space(6.0);

                                let pill_w = 100.0;
                                let pill_h = 22.0;
                                let (pill_rect, _) = ui.allocate_exact_size(egui::vec2(pill_w, pill_h), egui::Sense::hover());
                                ui.painter().rect_filled(pill_rect, 12.0, theme::ACCENT_SOFT);
                                ui.painter().rect_stroke(pill_rect, 12.0, egui::Stroke::new(1.0, theme::ACCENT_RING), egui::StrokeKind::Outside);
                                ui.painter().text(pill_rect.center(), egui::Align2::CENTER_CENTER, "Version 1.0.0", egui::FontId::proportional(11.5), theme::ACCENT);

                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("\u{201C}The VLC of image viewers.\u{201D}").italics().size(13.0));
                                ui.add_space(2.0);
                                ui.label(egui::RichText::new("Opens every image. 45+ formats, one window.").color(theme::TEXT_DIM).size(12.0));

                                ui.add_space(12.0);

                                let formats = [
                                    "JPEG", "PNG", "GIF", "WebP", "AVIF", "HEIC", "HEIF",
                                    "SVG", "PSD", "TIFF", "BMP", "ICO", "QOI", "EXR", "HDR",
                                    "TGA", "DDS", "PNM", "JXL", "JP2", "PCX", "XBM", "XPM",
                                    "SGI", "CR2", "NEF", "ARW", "DNG", "RAW", "farbfeld",
                                ];

                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
                                    for fmt in &formats {
                                        let btn = egui::Button::new(
                                            egui::RichText::new(*fmt).color(theme::TEXT_DIM).size(9.5),
                                        )
                                        .fill(theme::SURFACE_2)
                                        .stroke(egui::Stroke::new(1.0, theme::BORDER))
                                        .corner_radius(5.0)
                                        .sense(egui::Sense::hover());
                                        ui.add(btn);
                                    }
                                });

                                ui.add_space(12.0);
                                ui.label(egui::RichText::new("Built with Rust + egui \u{00B7} License AGPL-3.0").color(theme::TEXT_FAINT).size(10.5));
                                ui.add_space(12.0);
                            });
                        });
                });

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.show_about = false;
            }
        }
    }
}

impl eframe::App for CoveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.icon_texture.is_none() {
            let icon_bytes = include_bytes!("../docs/cove_icon.png");
            if let Ok(img) = image::load_from_memory(icon_bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels: Vec<egui::Color32> = rgba
                    .pixels()
                    .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                    .collect();
                let color_image = egui::ColorImage { size, pixels };
                self.icon_texture = Some(ctx.load_texture(
                    "cove_icon",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        if let Some(decoded) = self.pending_image.take() {
            let pixels_copy = decoded.pixels.clone();
            let texture = ctx.load_texture(
                "current_image",
                decoded.pixels,
                egui::TextureOptions::LINEAR,
            );
            self.current_texture = Some(texture);
            self.current_pixels = Some(pixels_copy);
        }

        if !self.anim_frames.is_empty() && self.anim_textures.is_empty() {
            for (i, (pixels, _)) in self.anim_frames.iter().enumerate() {
                let tex = ctx.load_texture(
                    format!("anim_frame_{i}"),
                    pixels.clone(),
                    egui::TextureOptions::LINEAR,
                );
                self.anim_textures.push(tex);
            }
            if let Some(tex) = self.anim_textures.first() {
                self.current_texture = Some(tex.clone());
                self.current_pixels = Some(self.anim_frames[0].0.clone());
            }
            self.anim_index = 0;
            self.anim_timer = 0.0;
            ctx.request_repaint();
        } else if self.anim_textures.len() > 1 {
            let dt = ctx.input(|i| i.unstable_dt).min(0.1);
            self.anim_timer += dt;
            let delay_s = self.anim_frames[self.anim_index].1 as f32 / 1000.0;
            if self.anim_timer >= delay_s {
                self.anim_timer -= delay_s;
                self.anim_index = (self.anim_index + 1) % self.anim_textures.len();
                self.current_texture = Some(self.anim_textures[self.anim_index].clone());
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

        self.handle_dropped_files(ctx);
        self.handle_keys(ctx);
        self.update_slideshow(ctx);

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            "Cove Image Viewer v1.0.0".to_string(),
        ));

        self.draw_titlebar(ctx);
        self.draw_menu_bar(ctx);
        self.draw_toolbar(ctx);
        self.draw_status_bar(ctx);
        self.draw_canvas(ctx);
        self.draw_dialogs(ctx);

        if self.pending_crop {
            self.pending_crop = false;
            self.crop_to_selection(ctx);
        }
        if self.pending_undo {
            self.pending_undo = false;
            self.undo_crop(ctx);
        }
    }
}

fn apply_transform(img: &egui::ColorImage, rotation: i32, flip_h: bool, flip_v: bool) -> egui::ColorImage {
    let w = img.width();
    let h = img.height();

    let get = |x: usize, y: usize| -> egui::Color32 {
        let (mut sx, mut sy) = (x, y);
        if flip_h { sx = w - 1 - sx; }
        if flip_v { sy = h - 1 - sy; }
        img.pixels[sy * w + sx]
    };

    match rotation {
        0 => {
            let mut pixels = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    pixels.push(get(x, y));
                }
            }
            egui::ColorImage { size: [w, h], pixels }
        }
        90 => {
            let mut pixels = Vec::with_capacity(w * h);
            for x in 0..w {
                for y in (0..h).rev() {
                    pixels.push(get(x, y));
                }
            }
            egui::ColorImage { size: [h, w], pixels }
        }
        180 => {
            let mut pixels = Vec::with_capacity(w * h);
            for y in (0..h).rev() {
                for x in (0..w).rev() {
                    pixels.push(get(x, y));
                }
            }
            egui::ColorImage { size: [w, h], pixels }
        }
        270 => {
            let mut pixels = Vec::with_capacity(w * h);
            for x in (0..w).rev() {
                for y in 0..h {
                    pixels.push(get(x, y));
                }
            }
            egui::ColorImage { size: [h, w], pixels }
        }
        _ => img.clone(),
    }
}

fn format_timestamp(secs: u64) -> String {
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let mut y = 1970u64;
    let mut d = days;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if d < days_in_year { break; }
        d -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [31, if leap {29} else {28}, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    for md in &month_days {
        if d < *md as u64 { break; }
        d -= *md as u64;
        m += 1;
    }
    format!("{y}-{:02}-{:02} {:02}:{:02}", m + 1, d + 1, hours, minutes)
}
