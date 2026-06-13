mod app;
mod autocrop;
mod browser;
mod cache;
mod decoder;
mod portable;
mod theme;
mod viewer;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cove", about = "The VLC of image viewers")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Image or directory to open
    path: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Auto-crop images by removing uniform background borders
    AutoCrop {
        /// Input directory or file (default: current directory)
        #[arg(default_value = ".")]
        input: PathBuf,

        /// Output directory (default: <input>/cropped/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Background color tolerance per channel (0-255)
        #[arg(short, long, default_value_t = 30)]
        tolerance: u8,

        /// Padding pixels around detected content
        #[arg(short, long, default_value_t = 4)]
        padding: u32,

        /// Preview crop results without writing files
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> eframe::Result<()> {
    // Force X11/XWayland backend on Linux. Winit has full XDND drag-and-drop
    // on X11 but none on native Wayland. Winit picks Wayland when WAYLAND_DISPLAY
    // is set, so we unset it to fall through to DISPLAY (XWayland).
    #[cfg(target_os = "linux")]
    if std::env::var("COVE_NATIVE_WAYLAND").is_err() {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    let cli = Cli::parse();

    if let Some(Command::AutoCrop { input, output, tolerance, padding, dry_run }) = cli.command {
        let ok = run_auto_crop(input, output, tolerance, padding, dry_run);
        std::process::exit(if ok { 0 } else { 1 });
    }

    let icon_bytes = include_bytes!("../docs/cove_icon.png");
    let icon = eframe::icon_data::from_png_bytes(icon_bytes).expect("failed to load icon");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Cove Image Viewer v1.1.0")
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

fn run_auto_crop(input: PathBuf, output: Option<PathBuf>, tolerance: u8, padding: u32, dry_run: bool) -> bool {
    let opts = autocrop::AutoCropOptions { tolerance, padding };

    if input.is_file() {
        let fname = match input.file_name() {
            Some(f) => f,
            None => { eprintln!("invalid input path: {}", input.display()); return false; }
        };
        let out = match output {
            Some(dir) => {
                if !dry_run {
                    let _ = std::fs::create_dir_all(&dir);
                }
                dir.join(fname)
            }
            None => {
                let stem = fname.to_string_lossy();
                let stem = stem.rsplit_once('.').map(|(s, _)| s).unwrap_or(&stem);
                let ext = input.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "png".into());
                input.with_file_name(format!("{}_cropped.{}", stem, ext))
            }
        };

        if dry_run {
            match image::open(&input) {
                Ok(img) => {
                    let img = img.to_rgba8();
                    match autocrop::smart_crop_bounds(&img, &opts) {
                        Some(r) => println!(
                            "{}: {}x{} -> {}x{} (would save {:.0}%)",
                            input.display(), r.original_width, r.original_height,
                            r.width, r.height,
                            (1.0 - (r.width as f64 * r.height as f64) / (r.original_width as f64 * r.original_height as f64)) * 100.0
                        ),
                        None => println!("{}: entirely background, skipping", input.display()),
                    }
                }
                Err(e) => { eprintln!("{}: {}", input.display(), e); return false; }
            }
        } else {
            if out.exists() {
                eprintln!("{}: output already exists, skipping", out.display());
                return false;
            }
            match autocrop::auto_crop_file(&input, &out, &opts) {
                Ok(r) => println!(
                    "{}: {}x{} -> {}x{} => {}",
                    input.display(), r.original_width, r.original_height,
                    r.width, r.height, out.display()
                ),
                Err(e) => { eprintln!("{}", e); return false; }
            }
        }
        return true;
    }

    let output_dir = output.unwrap_or_else(|| input.join("cropped"));

    if dry_run {
        println!("DRY RUN - no files will be written\n");
    } else {
        println!("Output: {}\n", output_dir.display());
    }

    let results = autocrop::batch_auto_crop(&input, &output_dir, &opts, dry_run);

    let mut cropped = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for (name, result) in &results {
        match result {
            Ok(r) => {
                let saved = (1.0 - (r.width as f64 * r.height as f64) / (r.original_width as f64 * r.original_height as f64)) * 100.0;
                println!("  {} {}x{} -> {}x{} ({:.0}% trimmed)", name, r.original_width, r.original_height, r.width, r.height, saved);
                cropped += 1;
            }
            Err(e) => {
                eprintln!("  {} SKIP: {}", name, e);
                if e.contains("background") { skipped += 1; } else { errors += 1; }
            }
        }
    }

    println!("\n{} cropped, {} skipped, {} errors", cropped, skipped, errors);
    errors == 0
}
