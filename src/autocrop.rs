use image::{RgbaImage, Rgba};
use std::path::Path;

#[derive(Clone)]
pub struct AutoCropOptions {
    pub tolerance: u8,
    pub padding: u32,
}

impl Default for AutoCropOptions {
    fn default() -> Self {
        Self {
            tolerance: 30,
            padding: 4,
        }
    }
}

pub struct CropResult {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
}

fn detect_background(img: &RgbaImage) -> Rgba<u8> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Rgba([0, 0, 0, 0]);
    }

    let sample_size = 8.min(w).min(h);
    let mut counts: std::collections::HashMap<[u8; 4], usize> = std::collections::HashMap::new();

    let corners: [(u32, u32); 4] = [(0, 0), (w - sample_size, 0), (0, h - sample_size), (w - sample_size, h - sample_size)];
    for &(cx, cy) in &corners {
        for dy in 0..sample_size {
            for dx in 0..sample_size {
                let p = img.get_pixel(cx + dx, cy + dy);
                *counts.entry(p.0).or_insert(0) += 1;
            }
        }
    }

    let best = counts.into_iter().max_by_key(|&(_, c)| c).unwrap().0;
    Rgba(best)
}

fn is_background(pixel: &Rgba<u8>, bg: &Rgba<u8>, tolerance: u8) -> bool {
    if pixel[3] == 0 {
        return true;
    }
    let tol = tolerance as i16;
    (pixel[0] as i16 - bg[0] as i16).abs() <= tol
        && (pixel[1] as i16 - bg[1] as i16).abs() <= tol
        && (pixel[2] as i16 - bg[2] as i16).abs() <= tol
        && (pixel[3] as i16 - bg[3] as i16).abs() <= tol
}

pub fn find_content_bounds(img: &RgbaImage, opts: &AutoCropOptions) -> Option<CropResult> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }

    let bg = detect_background(img);

    let mut is_content = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            if !is_background(img.get_pixel(x, y), &bg, opts.tolerance) {
                is_content[(y * w + x) as usize] = true;
            }
        }
    }

    let mut row_counts = vec![0u32; h as usize];
    let mut col_counts = vec![0u32; w as usize];
    for y in 0..h {
        for x in 0..w {
            if is_content[(y * w + x) as usize] {
                row_counts[y as usize] += 1;
                col_counts[x as usize] += 1;
            }
        }
    }

    // Two-pass approach:
    // Pass 1: Find the dense core of the sprite using a high threshold (5%).
    //         This locates the main body, ignoring thin elements and stray artifacts.
    // Pass 2: From the core, expand outward to include any non-background content
    //         reachable without crossing large empty gaps. This captures thin elements
    //         like thrown swords, hair tips, and weapon trails that are part of the
    //         sprite but too thin to meet the density threshold alone.

    let row_threshold = (w as f32 * 0.05).max(2.0) as u32;
    let col_threshold = (h as f32 * 0.05).max(2.0) as u32;

    // Pass 1: find core rows/cols. If no row/col meets the density threshold
    // (sparse sprites like 1px lines), fall back to any row/col with content.
    let core_row_start = row_counts.iter().position(|&c| c >= row_threshold)
        .or_else(|| row_counts.iter().position(|&c| c > 0))?;
    let core_row_end = row_counts.iter().rposition(|&c| c >= row_threshold)
        .or_else(|| row_counts.iter().rposition(|&c| c > 0))?;
    let core_col_start = col_counts.iter().position(|&c| c >= col_threshold)
        .or_else(|| col_counts.iter().position(|&c| c > 0))?;
    let core_col_end = col_counts.iter().rposition(|&c| c >= col_threshold)
        .or_else(|| col_counts.iter().rposition(|&c| c > 0))?;

    // Pass 2: expand from the core outward. A row/column is included if it has
    // ANY content (>=1 pixel) and there's no gap of entirely-empty rows/columns
    // wider than max_gap between it and the core. This bridges thin elements
    // near the sprite while still rejecting isolated artifacts far away.
    let row_gap_limit = (h as f32 * 0.05).max(3.0) as usize;
    let col_gap_limit = (w as f32 * 0.05).max(3.0) as usize;

    // Expand rows upward from core
    let mut min_y = core_row_start;
    let mut gap = 0usize;
    for r in (0..core_row_start).rev() {
        if row_counts[r] > 0 {
            min_y = r;
            gap = 0;
        } else {
            gap += 1;
            if gap > row_gap_limit { break; }
        }
    }

    // Expand rows downward from core
    let mut max_y = core_row_end;
    gap = 0;
    for r in (core_row_end + 1)..h as usize {
        if row_counts[r] > 0 {
            max_y = r;
            gap = 0;
        } else {
            gap += 1;
            if gap > row_gap_limit { break; }
        }
    }

    // Expand cols leftward from core
    let mut min_x_bound = core_col_start;
    gap = 0;
    for c in (0..core_col_start).rev() {
        if col_counts[c] > 0 {
            min_x_bound = c;
            gap = 0;
        } else {
            gap += 1;
            if gap > col_gap_limit { break; }
        }
    }

    // Expand cols rightward from core
    let mut max_x_bound = core_col_end;
    gap = 0;
    for c in (core_col_end + 1)..w as usize {
        if col_counts[c] > 0 {
            max_x_bound = c;
            gap = 0;
        } else {
            gap += 1;
            if gap > col_gap_limit { break; }
        }
    }

    // Final tight pixel-level bounds within the expanded region
    let mut min_x = w;
    let mut max_x = 0u32;
    let mut tight_min_y = h;
    let mut tight_max_y = 0u32;

    for y in (min_y as u32)..=(max_y as u32) {
        for x in (min_x_bound as u32)..=(max_x_bound as u32) {
            if is_content[(y * w + x) as usize] {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                tight_min_y = tight_min_y.min(y);
                tight_max_y = tight_max_y.max(y);
            }
        }
    }

    if max_x < min_x {
        return None;
    }

    let x1 = min_x.saturating_sub(opts.padding);
    let y1 = tight_min_y.saturating_sub(opts.padding);
    let x2 = max_x.saturating_add(1).saturating_add(opts.padding).min(w);
    let y2 = tight_max_y.saturating_add(1).saturating_add(opts.padding).min(h);

    Some(CropResult {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
        original_width: w,
        original_height: h,
    })
}

fn focus_crop(img: &RgbaImage) -> Option<CropResult> {
    let (w, h) = img.dimensions();
    if w < 64 || h < 64 {
        return None;
    }

    // Saliency-based subject detection: find the region that differs most from the
    // border colors. The border represents "background" — the subject stands out by
    // having different colors than the image edges.
    let step = 4u32;
    let sw = w / step;
    let sh = h / step;
    if sw < 4 || sh < 4 {
        return None;
    }

    // Compute border mean color (top/bottom/left/right strips)
    let margin_x = (w / 10).max(1);
    let margin_y = (h / 10).max(1);
    let mut border_r = 0i64;
    let mut border_g = 0i64;
    let mut border_b = 0i64;
    let mut border_n = 0i64;

    let mut add_pixel = |x: u32, y: u32, r: &mut i64, g: &mut i64, b: &mut i64, n: &mut i64| {
        let p = img.get_pixel(x, y);
        *r += p[0] as i64;
        *g += p[1] as i64;
        *b += p[2] as i64;
        *n += 1;
    };

    for y in (0..margin_y).step_by(step as usize) {
        for x in (0..w).step_by(step as usize) {
            add_pixel(x, y, &mut border_r, &mut border_g, &mut border_b, &mut border_n);
        }
    }
    for y in (h.saturating_sub(margin_y)..h).step_by(step as usize) {
        for x in (0..w).step_by(step as usize) {
            add_pixel(x, y, &mut border_r, &mut border_g, &mut border_b, &mut border_n);
        }
    }
    for y in (0..h).step_by(step as usize) {
        for x in (0..margin_x).step_by(step as usize) {
            add_pixel(x, y, &mut border_r, &mut border_g, &mut border_b, &mut border_n);
        }
        for x in (w - margin_x..w).step_by(step as usize) {
            add_pixel(x, y, &mut border_r, &mut border_g, &mut border_b, &mut border_n);
        }
    }

    if border_n == 0 {
        return None;
    }

    let bg_r = (border_r / border_n) as f32;
    let bg_g = (border_g / border_n) as f32;
    let bg_b = (border_b / border_n) as f32;

    // Combined subject score: saliency × edge density × center weight.
    // A pixel scores high only if it's different from the border AND has detail
    // AND is near the center. This filters out: plain backgrounds (no edges),
    // busy but peripheral content (low center weight), and textured background
    // that matches the border color (low saliency).
    let cx = sw as f64 / 2.0;
    let cy = sh as f64 / 2.0;
    let sigma_x = sw as f64 / 5.0;
    let sigma_y = sh as f64 / 5.0;

    // First pass: compute saliency and edge values to find their maxima
    let sws = sw as usize;
    let shs = sh as usize;
    let mut sal_map = vec![0.0f32; sws * shs];
    let mut edge_map = vec![0.0f32; sws * shs];

    for sy in 0..sh {
        for sx in 0..sw {
            let px = (sx * step).min(w - 1);
            let py = (sy * step).min(h - 1);
            let p = img.get_pixel(px, py);
            let dr = p[0] as f32 - bg_r;
            let dg = p[1] as f32 - bg_g;
            let db = p[2] as f32 - bg_b;
            sal_map[sy as usize * sws + sx as usize] = (dr * dr + dg * dg + db * db).sqrt();

            let luma = |x: u32, y: u32| -> f32 {
                let q = img.get_pixel(x.min(w - 1), y.min(h - 1));
                q[0] as f32 * 0.299 + q[1] as f32 * 0.587 + q[2] as f32 * 0.114
            };
            let here = luma(px, py);
            let dx = (luma((px + step).min(w - 1), py) - here).abs();
            let dy = (luma(px, (py + step).min(h - 1)) - here).abs();
            edge_map[sy as usize * sws + sx as usize] = dx + dy;
        }
    }

    let sal_max = sal_map.iter().cloned().fold(0.0f32, f32::max).max(1.0);
    let edge_max = edge_map.iter().cloned().fold(0.0f32, f32::max).max(1.0);

    // Second pass: combine normalized saliency × normalized edges × center weight
    let mut col_score = vec![0.0f64; sws];
    let mut row_score = vec![0.0f64; shs];

    for sy in 0..sh {
        let wy = (-(((sy as f64) - cy).powi(2)) / (2.0 * sigma_y * sigma_y)).exp();
        for sx in 0..sw {
            let wx = (-(((sx as f64) - cx).powi(2)) / (2.0 * sigma_x * sigma_x)).exp();
            let idx = sy as usize * sws + sx as usize;
            let sal_n = sal_map[idx] / sal_max;
            let edge_n = edge_map[idx] / edge_max;
            let score = sal_n as f64 * edge_n as f64 * wx * wy;
            col_score[sx as usize] += score;
            row_score[sy as usize] += score;
        }
    }

    // Adaptive window using cumulative score percentiles (15th-85th).
    // Tighter than 10th-90th because the multiplicative score is already
    // more concentrated than saliency alone.
    let adaptive_window = |density: &[f64]| -> Option<(usize, usize)> {
        let len = density.len();
        if len < 4 {
            return None;
        }

        let total: f64 = density.iter().sum();
        if total <= 0.0 {
            return None;
        }

        let lo_target = total * 0.15;
        let hi_target = total * 0.85;

        let mut cumsum = 0.0f64;
        let mut start = 0usize;
        let mut end = len;
        let mut found_start = false;

        for i in 0..len {
            cumsum += density[i];
            if !found_start && cumsum >= lo_target {
                start = i;
                found_start = true;
            }
            if cumsum >= hi_target {
                end = i + 1;
                break;
            }
        }

        // Add margin (5%) for breathing room
        let margin = (len / 20).max(1);
        let start = start.saturating_sub(margin);
        let end = (end + margin).min(len);

        // Must crop at least 15% on this axis to be worth it
        if end - start > len * 85 / 100 {
            return None;
        }

        Some((start, end))
    };

    let col_range = adaptive_window(&col_score);
    let row_range = adaptive_window(&row_score);

    // Need at least one axis to crop on
    if col_range.is_none() && row_range.is_none() {
        return None;
    }

    let (col_start, col_end) = col_range.unwrap_or((0, sw as usize));
    let (row_start, row_end) = row_range.unwrap_or((0, sh as usize));

    let x1 = (col_start as u32 * step).min(w);
    let x2 = (col_end as u32 * step).min(w);
    let y1 = (row_start as u32 * step).min(h);
    let y2 = (row_end as u32 * step).min(h);

    let cw = x2 - x1;
    let ch = y2 - y1;

    let area_ratio = (cw as f64 * ch as f64) / (w as f64 * h as f64);
    if area_ratio > 0.90 {
        return None;
    }

    Some(CropResult {
        x: x1,
        y: y1,
        width: cw,
        height: ch,
        original_width: w,
        original_height: h,
    })
}

pub fn smart_crop_bounds(img: &RgbaImage, opts: &AutoCropOptions) -> Option<CropResult> {
    let bounds = find_content_bounds(img, opts);

    match bounds {
        Some(ref b) => {
            let area_ratio = (b.width as f64 * b.height as f64)
                / (b.original_width as f64 * b.original_height as f64);
            if area_ratio > 0.90 {
                // Background removal barely cropped - try focus crop for photos.
                // But only if the image has real variance (not uniform/blank).
                focus_crop(img).or(bounds)
            } else {
                bounds
            }
        }
        // No foreground found at all - image is entirely background/uniform.
        // Do not fall through to focus_crop (which would blindly crop 10-30%).
        None => None,
    }
}

pub fn auto_crop_image(img: &RgbaImage, opts: &AutoCropOptions) -> Option<(RgbaImage, CropResult)> {
    let bounds = smart_crop_bounds(img, opts)?;
    let cropped = image::imageops::crop_imm(img, bounds.x, bounds.y, bounds.width, bounds.height).to_image();
    Some((cropped, bounds))
}

pub fn auto_crop_file(
    input: &Path,
    output: &Path,
    opts: &AutoCropOptions,
) -> Result<CropResult, String> {
    let canon_in = std::fs::canonicalize(input).unwrap_or_else(|_| input.to_path_buf());
    let canon_out = std::fs::canonicalize(output).unwrap_or_else(|_| output.to_path_buf());
    if canon_in == canon_out {
        return Err(format!("{}: output path is the same as input", input.display()));
    }

    let img = image::open(input)
        .map_err(|e| format!("failed to open {}: {}", input.display(), e))?
        .to_rgba8();

    let (cropped, result) = auto_crop_image(&img, opts)
        .ok_or_else(|| format!("{}: image is entirely background", input.display()))?;

    if result.width == result.original_width && result.height == result.original_height {
        std::fs::copy(input, output)
            .map_err(|e| format!("failed to copy {}: {}", input.display(), e))?;
        return Ok(result);
    }

    let dyn_img = image::DynamicImage::ImageRgba8(cropped);
    let is_jpeg = output.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "jpg" | "jpeg"))
        .unwrap_or(false);
    let to_save = if is_jpeg { dyn_img.to_rgb8().into() } else { dyn_img };
    to_save.save(output)
        .map_err(|e| format!("failed to save {}: {}", output.display(), e))?;

    Ok(result)
}

pub fn auto_crop_egui(pixels: &egui::ColorImage, opts: &AutoCropOptions) -> Option<(egui::ColorImage, CropResult)> {
    let w = pixels.width() as u32;
    let h = pixels.height() as u32;
    let rgba: Vec<u8> = pixels.pixels.iter().flat_map(|c| c.to_array()).collect();
    let img = RgbaImage::from_raw(w, h, rgba)?;

    let bounds = smart_crop_bounds(&img, opts)?;

    if bounds.width == w && bounds.height == h {
        return None;
    }

    let x1 = bounds.x as usize;
    let y1 = bounds.y as usize;
    let cw = bounds.width as usize;
    let ch = bounds.height as usize;
    let pw = w as usize;

    let mut cropped_pixels = Vec::with_capacity(cw * ch);
    for y in y1..(y1 + ch) {
        for x in x1..(x1 + cw) {
            cropped_pixels.push(pixels.pixels[y * pw + x]);
        }
    }

    Some((
        egui::ColorImage {
            size: [cw, ch],
            pixels: cropped_pixels,
        },
        bounds,
    ))
}

pub fn batch_auto_crop(
    input_dir: &Path,
    output_dir: &Path,
    opts: &AutoCropOptions,
    dry_run: bool,
) -> Vec<(String, Result<CropResult, String>)> {
    let extensions = [
        "png", "jpg", "jpeg", "bmp", "tiff", "tif", "pnm", "pbm", "pgm", "ppm",
    ];

    let mut entries: Vec<_> = match std::fs::read_dir(input_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| extensions.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
            })
            .collect(),
        Err(e) => {
            return vec![("(directory)".into(), Err(format!("failed to read directory: {}", e)))];
        }
    };
    entries.sort_by_key(|e| e.file_name());

    if !dry_run {
        let canon_in = std::fs::canonicalize(input_dir).unwrap_or_else(|_| input_dir.to_path_buf());
        let canon_out = if output_dir.exists() {
            std::fs::canonicalize(output_dir).unwrap_or_else(|_| output_dir.to_path_buf())
        } else {
            output_dir.to_path_buf()
        };
        if canon_in == canon_out {
            return vec![("(output dir)".into(), Err("output directory cannot be the same as input directory".into()))];
        }
        if let Err(e) = std::fs::create_dir_all(output_dir) {
            return vec![("(output dir)".into(), Err(format!("failed to create output directory: {}", e)))];
        }
    }

    let mut results = Vec::new();

    for entry in &entries {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let out_path = output_dir.join(&name);

        if dry_run {
            match image::open(&path) {
                Ok(img) => {
                    let img = img.to_rgba8();
                    match smart_crop_bounds(&img, opts) {
                        Some(r) => results.push((name, Ok(r))),
                        None => results.push((name, Err("entirely background".into()))),
                    }
                }
                Err(e) => results.push((name, Err(format!("failed to open: {}", e)))),
            }
        } else {
            if out_path.exists() {
                results.push((name, Err("output already exists, skipping".into())));
            } else {
                results.push((name, auto_crop_file(&path, &out_path, opts)));
            }
        }
    }

    results
}
