mod app;
mod browser;
mod decoder;
mod theme;
mod viewer;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cove", about = "The VLC of image viewers")]
struct Cli {
    path: Option<PathBuf>,
}

fn main() -> eframe::Result<()> {
    let cli = Cli::parse();

    let icon_bytes = include_bytes!("../docs/cove_icon.png");
    let icon = eframe::icon_data::from_png_bytes(icon_bytes).expect("failed to load icon");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Cove Image Viewer v1.0.0")
            .with_icon(icon)
            .with_decorations(false)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "cove-image-viewer",
        options,
        Box::new(|cc| {
            theme::apply_cove_theme(&cc.egui_ctx);
            Ok(Box::new(app::CoveApp::new(cli.path)))
        }),
    )
}
