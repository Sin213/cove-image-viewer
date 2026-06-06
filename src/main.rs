mod app;
mod browser;
mod decoder;
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Cove Image Viewer")
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "cove-image-viewer",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(app::CoveApp::new(cli.path)))
        }),
    )
}
