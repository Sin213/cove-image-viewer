use crate::browser::Browser;
use crate::decoder::{self, DecodedImage};
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

        match decoder::load_image(path) {
            Ok(decoded) => {
                self.viewer.reset_for_new_image();
                self.current_path = Some(path.to_path_buf());
                self.current_image_size = Vec2::new(
                    decoded.original_width as f32,
                    decoded.original_height as f32,
                );
                self.current_format = decoded.format_name.clone();
                self.current_file_size = file_size;
                self.error = None;
                self.pending_image = Some(decoded);
            }
            Err(e) => {
                self.error = Some((path.to_path_buf(), e));
                self.current_texture = None;
                self.current_path = Some(path.to_path_buf());
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
                    "pbm", "ff", "cr2", "nef", "arw", "dng",
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
                if self.slideshow_active {
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

        for t in &text_actions {
            match t.as_str() {
                "+" | "=" => self.viewer.zoom_in(),
                "0" => self.viewer.set_fit_mode(FitMode::FitWindow),
                "1" => self.viewer.set_fit_mode(FitMode::ActualSize),
                "f" => self.viewer.cycle_fit_mode(),
                "i" => self.show_info = !self.show_info,
                "r" => self.viewer.rotate_cw(),
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

    fn draw_status_bar(&self, ctx: &egui::Context) {
        if self.fullscreen && !self.show_info {
            return;
        }

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.current_texture.is_none() && self.error.is_none() {
                    ui.label("No image loaded — Ctrl+O to open, or drag & drop");
                    return;
                }

                if let Some((path, msg)) = &self.error {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    ui.colored_label(egui::Color32::RED, format!("{name}: {msg}"));
                    ui.separator();
                    ui.label(self.browser.position_label());
                    return;
                }

                if let Some(path) = &self.current_path {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    ui.label(name);
                    ui.separator();
                    ui.label(format!(
                        "{}x{}",
                        self.current_image_size.x as u32,
                        self.current_image_size.y as u32
                    ));
                    ui.separator();
                    ui.label(&self.current_format);
                    ui.separator();
                    ui.label(decoder::format_file_size(self.current_file_size));
                    ui.separator();
                    ui.label(format!("{:.0}%", self.viewer.zoom_percent()));
                    ui.separator();
                    ui.label(self.browser.position_label());
                    if self.slideshow_active {
                        ui.separator();
                        ui.label("▶ Slideshow");
                    }
                }
            });
        });
    }

    fn draw_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(26, 26, 26)))
            .show(ctx, |ui| {
                if let Some(ref texture) = self.current_texture {
                    let tex = texture.clone();
                    let size = self.current_image_size;
                    self.viewer.paint(ui, &tex, size);
                } else if let Some((_, msg)) = &self.error {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 100, 100),
                            format!("Failed to load: {msg}"),
                        );
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.heading("Drop an image here or press Ctrl+O");
                    });
                }
            });
    }
}

impl eframe::App for CoveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(decoded) = self.pending_image.take() {
            let texture = ctx.load_texture(
                "current_image",
                decoded.pixels,
                egui::TextureOptions::LINEAR,
            );
            self.current_texture = Some(texture);
        }

        self.handle_dropped_files(ctx);
        self.handle_keys(ctx);
        self.update_slideshow(ctx);

        if let Some(path) = &self.current_path {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Cove");
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "{name} — Cove Image Viewer"
            )));
        }

        self.draw_status_bar(ctx);
        self.draw_canvas(ctx);
    }
}
