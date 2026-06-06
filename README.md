# Cove Image Viewer

The VLC of image viewers — opens everything.

A fast, cross-platform image viewer built in Rust with egui. Single 22 MB binary with zero runtime dependencies. Supports 45+ image formats including HEIC, AVIF, RAW, PSD, SVG, JPEG XL, JPEG 2000, and more.

## Supported Formats

| Category | Formats |
|----------|---------|
| **Common** | JPEG, PNG, GIF, BMP, TIFF, WebP |
| **Modern** | AVIF, HEIC/HEIF, JPEG XL, QOI |
| **Professional** | PSD/PSB, OpenEXR, HDR (Radiance), DDS, TGA |
| **Vector** | SVG |
| **Camera RAW** | CR2, CRW, NEF, NRW, ARW, DNG, ORF, RW2, RAF, PEF, SRW, MRW, ERF, SR2, SRF, X3F, 3FR, and more (30+ cameras) |
| **Legacy/Niche** | PCX, XBM, XPM, SGI/RGB/BW, ICO, CUR, PNM/PPM/PGM/PBM, farbfeld |
| **JPEG 2000** | JP2, J2K, J2C, JPX |

## Building

### Prerequisites

- Rust toolchain (1.89+)
- A C compiler (for openjp2 build)

That's it — all image decoders are compiled from source. No system libraries needed.

### Build

```bash
cargo build --release
# Binary is at target/release/cove-image-viewer
```

## Usage

```bash
# Open a single image
cove path/to/image.jpg

# Open a directory (browse all images in it)
cove path/to/photos/

# No arguments — opens an empty window, drag & drop or Ctrl+O
cove
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| **Left / Right** | Previous / Next image |
| **Home / End** | First / Last image |
| **Scroll wheel** | Zoom in / out (cursor-anchored) |
| **Click + drag** | Pan |
| **+ / -** | Zoom in / out |
| **0** | Fit to window |
| **1** | Actual size (1:1) |
| **F** | Cycle fit modes |
| **F11** | Toggle fullscreen |
| **Escape** | Exit fullscreen / slideshow |
| **S** | Start / stop slideshow |
| **R** | Rotate 90° clockwise |
| **H** | Flip horizontal |
| **V** | Flip vertical |
| **I** | Toggle image info |
| **Ctrl+O** | Open file dialog |

## Features

- **Self-contained** — single binary, zero runtime dependencies
- **Fast** — GPU-accelerated rendering via OpenGL, <100ms startup
- **Lightweight** — 22 MB binary, ~30-50 MB RAM idle
- **Cross-platform** — Linux, macOS, Windows
- **Browse** — navigate all images in a directory with arrow keys, wraps around
- **Zoom & Pan** — smooth scroll-wheel zoom anchored at cursor, click-drag to pan
- **Fullscreen** — F11, hides all UI chrome
- **Slideshow** — automatic 5-second interval, auto-enters fullscreen
- **Drag & Drop** — drop any image onto the window
- **View transforms** — rotation, horizontal/vertical flip (view only, never modifies files)
- **Natural sort** — files sorted like a human would (img2 before img10)
- **Dark theme** — easy on the eyes

## Architecture

```
src/
├── main.rs      # CLI args, window setup
├── app.rs       # Main loop, keyboard handling, UI layout
├── decoder.rs   # Format detection + decoding (all 45+ formats)
├── viewer.rs    # Zoom, pan, rotation, GPU rendering
└── browser.rs   # Directory scanning, navigation, natural sort
```

## License

AGPL-3.0 — due to the pure-Rust HEIC decoder dependency.
