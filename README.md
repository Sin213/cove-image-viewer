# Cove Image Viewer

The VLC of image viewers — opens every image.

A fast, cross-platform image viewer built in Rust with egui. Single binary with zero runtime dependencies. Supports 60+ image formats including HEIC, AVIF, RAW, PSD, SVG, JPEG XL, JPEG 2000, and more.

Works on **Linux**, **Windows**, and **macOS**.

## Supported Formats

| Category | Formats |
|----------|---------|
| **Common** | JPEG, PNG, APNG, GIF, BMP, TIFF, WebP |
| **Modern** | AVIF, HEIC/HEIF, JPEG XL, QOI |
| **Professional** | PSD/PSB, OpenEXR, HDR (Radiance), DDS, TGA |
| **Vector** | SVG |
| **Camera RAW** | CR2, CRW, NEF, NRW, ARW, DNG, ORF, RW2, RAF, PEF, SRW, MRW, ERF, SR2, SRF, X3F, 3FR, and more (30+ cameras) |
| **Legacy/Niche** | PCX, XBM, XPM, SGI/RGB/BW, ICO, CUR, PNM/PPM/PGM/PBM, farbfeld |
| **JPEG 2000** | JP2, J2K, J2C, JPX |

## Features

- **60+ formats** — opens virtually anything, pure-Rust decoders, no system libraries
- **Async loading** — images decode in background threads, UI never freezes
- **LRU cache + prefetch** — decoded images cached, adjacent images pre-loaded for instant navigation
- **Save As / Convert** — export to PNG, JPEG, BMP, TIFF, WebP, GIF, TGA, QOI, PPM, farbfeld with quality/compression controls
- **EXIF metadata** — camera, lens, aperture, shutter speed, ISO, GPS, date taken (press I)
- **Compare view** — side-by-side image comparison with Ctrl+K
- **Slideshow** — configurable interval (2s-30s), auto-fullscreen
- **Crop & selection** — selection clamped to image bounds, crop with Ctrl+Y, zoom to selection
- **RAW preview** — extracts embedded JPEG preview for near-instant RAW file display
- **Animated images** — GIF, WebP, and APNG with proper frame timing
- **Zoom & pan** — smooth scroll-wheel zoom, right-click drag to pan
- **Rotation & flip** — rotate 90, flip horizontal/vertical, transforms saved on export
- **Cross-platform** — Linux, Windows, macOS from a single codebase
- **Self-contained** — single binary, zero runtime dependencies
- **Dark theme** — custom UI with teal accents, no traffic-light buttons

## Installation

### Pre-built binaries

Download from [Releases](../../releases) — available for Linux, Windows, and macOS.

### Build from source

```bash
# Prerequisites: Rust toolchain (1.89+) and a C compiler (for openjp2)
cargo build --release
# Binary is at target/release/cove-image-viewer
```

### Cross-platform builds

```bash
# Windows (from Linux)
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu

# macOS (from CI or a Mac)
cargo build --release --target aarch64-apple-darwin
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
| **Scroll wheel** | Zoom in / out |
| **Right-click drag** | Pan |
| **Left-click drag** | Selection rectangle |
| **+ / -** | Zoom in / out |
| **0** | Fit to window |
| **1** | Actual size (1:1) |
| **F** | Cycle fit modes |
| **F11** | Toggle fullscreen |
| **S** | Start / stop slideshow |
| **R / L** | Rotate clockwise / counter-clockwise |
| **H / V** | Flip horizontal / vertical |
| **I** | Image information + EXIF data |
| **Ctrl+O** | Open file |
| **Ctrl+Shift+S** | Save As / Convert |
| **Ctrl+Y** | Crop to selection |
| **Ctrl+K** | Compare view (set reference / toggle) |
| **Ctrl+C** | Copy to clipboard |
| **Ctrl+Z** | Undo crop |
| **Delete** | Delete file (with confirmation) |
| **Escape** | Exit fullscreen / slideshow / compare / clear selection |

## Architecture

```
src/
├── main.rs      # CLI args, window setup
├── app.rs       # Main loop, keyboard handling, UI layout, Save As, compare view
├── decoder.rs   # Format detection, decoding, EXIF reader, RAW preview extraction
├── viewer.rs    # Zoom, pan, rotation, selection, GPU rendering
├── browser.rs   # Directory scanning, navigation, natural sort
└── cache.rs     # LRU image cache for decoded images
```

## License

AGPL-3.0
