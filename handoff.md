# Handoff: Cove Image Viewer — Session 2026-06-06

## Scope
Five feature commits on top of `301fd8f`:

1. **Async image loading, LRU cache, prefetch, APNG** (`692f1c6`)
   - Moved all image decoding to background threads via mpsc channels
   - 20-entry LRU cache for decoded images
   - Prefetch adjacent images (N+1, N-1) after display
   - catch_unwind wrapper for decoder panics
   - APNG support in animated decoder
   - New file: `src/cache.rs`

2. **Format count update** (`28432b7`)
   - Updated "45+" to "60+" across UI (about dialog, welcome screen)

3. **EXIF metadata, RAW preview, drop overlay** (`c62735b`)
   - EXIF panel in Image Information dialog (camera, lens, aperture, shutter, ISO, GPS, etc.)
   - RAW files try embedded JPEG preview before full decode
   - Visual drop overlay when files dragged over window
   - Fixed 'I' hotkey to open info dialog instead of toggling status bar
   - New dep: `kamadak-exif`

4. **Slideshow interval, Save As, compare view** (`b263f81`)
   - Slideshow interval selector (2s–30s) in File menu
   - Save As (Ctrl+Shift+S) with format conversion (PNG/JPEG/BMP/TIFF/WebP/GIF/TGA/QOI/PPM/farbfeld)
   - JPEG quality slider and PNG compression level dialogs
   - Side-by-side compare view (Ctrl+K) with labeled filenames

5. **CI workflow** (`5a32b38`)
   - GitHub Actions matrix build for Linux, Windows, macOS
   - Auto-release with checksums on version tags

## Changed files
- `Cargo.toml` — new dep (kamadak-exif), panic=unwind
- `Cargo.lock` — lockfile update
- `src/main.rs` — added `mod cache`
- `src/cache.rs` — new: LRU image cache
- `src/decoder.rs` — Clone on DecodedImage, APNG, RAW preview extraction, EXIF reader
- `src/app.rs` — async loading, save-as, compare view, slideshow interval, EXIF display, drop overlay
- `.github/workflows/build.yml` — new: CI workflow

## Verification
- `cargo check` passes (only pre-existing warnings)
- `cargo build --release` passes
- Manual testing: navigation, format loading, EXIF display, save-as, compare view
