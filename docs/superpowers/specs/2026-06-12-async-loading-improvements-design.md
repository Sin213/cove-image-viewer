# Async Loading Improvements

Incremental improvements to the existing async image loading system in Cove Image Viewer. Four changes, all additive to the current architecture.

## 1. Memory-Based Cache (256 MB)

Replace the entry-count LRU in `cache.rs` with a byte-budget LRU.

- Each `CachedImage` tracks its pixel data size (`width * height * 4` bytes for RGBA; sum of all frames for animated images).
- `ImageCache` stores a `current_bytes: usize` running total and a `max_bytes: usize` cap (256 MB = `256 * 1024 * 1024`).
- `put()` evicts oldest entries until the new image fits within budget.
- `get()` still promotes to back of the queue (LRU touch).
- The `max_entries` field is removed. The `new()` constructor takes `max_bytes: usize`.
- API surface (`get`, `put`, `contains`) stays the same. Callers are unchanged.

### Size calculation

```
CachedImage::Static(d) => d.pixels.size[0] * d.pixels.size[1] * 4
CachedImage::Animated(frames) => frames.iter().map(|(img, _)| img.size[0] * img.size[1] * 4).sum()
```

## 2. Loading Indicator (Spinner + Filename)

When `loading_path` is `Some`, render a centered loading indicator on the canvas area.

- **Spinner**: A rotating arc drawn with `egui::Painter::arc()` or equivalent. Uses `theme::ACCENT` color (teal). Rotates based on `ctx.input(|i| i.time)` for smooth animation.
- **Filename**: The file stem (not full path) rendered below the spinner in the theme's text color. E.g. "DSC_0042.CR2".
- **Placement**: Centered in the canvas rect (`self.canvas_rect`).
- **Scope**: Only shown for the actively requested image (`loading_path`), not for prefetch loads.
- **Disappears**: The frame `apply_cached_image` clears `loading_path`.
- Requires `ctx.request_repaint()` while loading (already present at line 1652).

## 3. Cancel Stale Decodes

Add cancellation tokens to avoid wasting CPU on images the user has navigated past.

- New field on `CoveApp`: `cancel_token: Arc<AtomicBool>`.
- `load_image()` sets the previous token to `true` (cancelled) and creates a fresh `Arc<AtomicBool>` for the new load.
- The token is passed into the spawned thread alongside `path`, `tx`, etc.
- `decode_to_cached()` (and the underlying `decoder::load_image()`) accepts an `Option<&AtomicBool>` cancellation flag.
- The decoder checks the flag at natural breakpoints:
  - After format detection / content sniffing
  - After reading raw bytes from disk
  - Before expensive pixel conversion (color space, scaling)
- If cancelled, returns `Err("cancelled".into())`.
- In the `try_recv` loop, cancelled results are discarded (not cached).
- Prefetch loads pass `None` for the cancellation token since they are intentional cache warming.

### Changes to `LoadComplete`

Add a `cancelled: bool` field, or detect cancellation by checking `result == Err("cancelled")`.

## 4. Deeper Prefetch (+/- 3)

Expand prefetching from +/- 1 to +/- 3 with distance-based priority.

- Spawn order: +1, -1, +2, -2, +3, -3 (closest first).
- Skip images already in cache (`image_cache.contains`) or in-flight (`inflight_paths`).
- Cap concurrent in-flight prefetches: skip spawning if `inflight_paths.len() >= 4` (1 active + 3 prefetch max).
- Wrapping at folder boundaries stays the same as current behavior.
- Prefetch loads use generation `0` (same as current) so they are never treated as the "target" load.

## Files Changed

- `src/cache.rs` - byte-budget LRU eviction
- `src/app.rs` - cancellation tokens, spinner rendering, expanded prefetch, updated cache construction
- `src/decoder.rs` - accept optional cancellation token, check at breakpoints

## Not in Scope

- Configurable cache size (hardcoded 256 MB)
- Thumbnail/reduced-resolution decoding
- Progress percentage (decode is not incremental enough to report %)
- New modules or architectural changes
