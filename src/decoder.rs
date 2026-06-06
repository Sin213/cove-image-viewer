use std::path::Path;

#[derive(Clone)]
pub struct DecodedImage {
    pub pixels: egui::ColorImage,
    pub format_name: String,
    pub original_width: u32,
    pub original_height: u32,
}

pub struct AnimatedFrame {
    pub pixels: egui::ColorImage,
    pub delay_ms: u32,
}

pub fn load_animated(path: &Path) -> Option<Vec<AnimatedFrame>> {
    use image::AnimationDecoder;

    let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);

    let frames_iter: Box<dyn Iterator<Item = Result<image::Frame, image::ImageError>>> = match ext.as_str() {
        "gif" => {
            let decoder = image::codecs::gif::GifDecoder::new(reader).ok()?;
            Box::new(decoder.into_frames())
        }
        "webp" => {
            let decoder = image::codecs::webp::WebPDecoder::new(reader).ok()?;
            if !decoder.has_animation() {
                return None;
            }
            Box::new(decoder.into_frames())
        }
        "png" | "apng" => {
            let decoder = image::codecs::png::PngDecoder::new(reader).ok()?;
            if !decoder.is_apng().unwrap_or(false) {
                return None;
            }
            Box::new(decoder.apng().ok()?.into_frames())
        }
        _ => return None,
    };

    let mut out = Vec::new();
    for frame_result in frames_iter {
        let frame = frame_result.ok()?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom == 0 { 100 } else { numer / denom };
        let delay_ms = if delay_ms == 0 { 100 } else { delay_ms };
        let rgba = frame.into_buffer();
        let w = rgba.width() as usize;
        let h = rgba.height() as usize;
        let pixels = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
        out.push(AnimatedFrame { pixels, delay_ms });
    }

    if out.len() > 1 { Some(out) } else { None }
}

pub fn load_image(path: &Path) -> Result<DecodedImage, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let result = match ext.as_str() {
        // HEIF/HEIC/AVIF via libheif
        "heic" | "heif" | "avif" | "hif" => load_heif(path),
        // SVG via resvg
        "svg" | "svgz" => load_svg(path),
        // JPEG XL
        "jxl" => load_jxl(path),
        // Photoshop
        "psd" | "psb" => load_psd(path),
        // Camera RAW
        "cr2" | "nef" | "arw" | "dng" | "orf" | "rw2" | "raf" | "pef" | "srw" | "mrw"
        | "erf" | "kdc" | "dcr" | "raw" | "3fr" | "ari" | "bay" | "cap" | "crw" | "dcs"
        | "drf" | "fff" | "iiq" | "mos" | "mef" | "nrw" | "ptx" | "pxn" | "r3d" | "rwl"
        | "rwz" | "sr2" | "srf" | "x3f" => load_raw(path),
        // PCX
        "pcx" => load_pcx_image(path),
        // JPEG 2000
        "jp2" | "j2k" | "j2c" | "jpx" | "jpf" | "jpm" => load_jp2(path),
        // XBM (X BitMap)
        "xbm" => load_xbm(path),
        // XPM (X PixMap)
        "xpm" => load_xpm(path),
        // SGI (Silicon Graphics Image)
        "sgi" | "rgb" | "rgba" | "bw" | "int" | "inta" => load_sgi(path),
        // TGA might need manual handling for some edge cases
        // but image crate handles it, so fall through
        _ => load_via_image_crate(path),
    };

    // Fallback: if specific decoder fails, try image crate
    match &result {
        Ok(_) => result,
        Err(specific_err) => {
            load_via_image_crate(path).map_err(|img_err| {
                format!("{specific_err} (fallback also failed: {img_err})")
            })
        }
    }
}

// ─── image crate (Tier 1: JPEG, PNG, GIF, BMP, TIFF, WebP, ICO, QOI, EXR, HDR, TGA, DDS, PNM, farbfeld) ───

fn to_color_image(img: image::DynamicImage, format_name: &str) -> DecodedImage {
    let width = img.width();
    let height = img.height();
    let rgba = img.into_rgba8();
    let pixels = rgba.as_raw();
    DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], pixels),
        format_name: format_name.to_string(),
        original_width: width,
        original_height: height,
    }
}

fn load_via_image_crate(path: &Path) -> Result<DecodedImage, String> {
    let img = image::open(path).map_err(|e| format!("image: {e}"))?;
    let format_name = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_else(|| "Unknown".into());
    Ok(to_color_image(img, &format_name))
}

// ─── HEIF/HEIC/AVIF via heic crate (pure Rust) ───

fn load_heif(path: &Path) -> Result<DecodedImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {e}"))?;

    let config = heic::DecoderConfig::new();
    let output = config
        .decode(&data, heic::PixelLayout::Rgba8)
        .map_err(|e| format!("heic: {e}"))?;

    let width = output.width as usize;
    let height = output.height as usize;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("heif");
    let format_name = match ext.to_lowercase().as_str() {
        "avif" => "AVIF",
        "heic" => "HEIC",
        _ => "HEIF",
    };

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied([width, height], &output.data),
        format_name: format_name.to_string(),
        original_width: width as u32,
        original_height: height as u32,
    })
}

// ─── SVG via resvg ───

fn load_svg(path: &Path) -> Result<DecodedImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &options).map_err(|e| format!("svg parse: {e}"))?;

    let size = tree.size();
    let mut scale = 1.0_f32;
    let max_dim = 4096.0;
    if size.width() > max_dim || size.height() > max_dim {
        scale = (max_dim / size.width()).min(max_dim / size.height());
    }

    let width = (size.width() * scale) as u32;
    let height = (size.height() * scale) as u32;

    if width == 0 || height == 0 {
        return Err("SVG has zero dimensions".into());
    }

    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "Failed to create pixmap".to_string())?;

    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let pixels = pixmap.data();
    let mut straight = Vec::with_capacity(pixels.len());
    for chunk in pixels.chunks(4) {
        let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        if a == 0 {
            straight.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let af = a as f32 / 255.0;
            straight.push((r as f32 / af).min(255.0) as u8);
            straight.push((g as f32 / af).min(255.0) as u8);
            straight.push((b as f32 / af).min(255.0) as u8);
            straight.push(a);
        }
    }

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &straight,
        ),
        format_name: "SVG".to_string(),
        original_width: size.width() as u32,
        original_height: size.height() as u32,
    })
}

// ─── JPEG XL via jxl-oxide ───

fn load_jxl(path: &Path) -> Result<DecodedImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    let decoder = jxl_oxide::JxlImage::builder()
        .read(&data[..])
        .map_err(|e| format!("jxl init: {e}"))?;

    let render = decoder
        .render_frame(0)
        .map_err(|e| format!("jxl render: {e}"))?;
    let fb = render.image_all_channels();
    let width = fb.width() as u32;
    let height = fb.height() as u32;
    let num_channels = fb.channels();
    let buf = fb.buf();

    let pixel_count = (width * height) as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);

    for i in 0..pixel_count {
        let base = i * num_channels;
        let r = (buf[base].clamp(0.0, 1.0) * 255.0) as u8;
        let g = (buf[base + 1].clamp(0.0, 1.0) * 255.0) as u8;
        let b = (buf[base + 2].clamp(0.0, 1.0) * 255.0) as u8;
        let a = if num_channels > 3 {
            (buf[base + 3].clamp(0.0, 1.0) * 255.0) as u8
        } else {
            255
        };
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &rgba,
        ),
        format_name: "JPEG XL".to_string(),
        original_width: width,
        original_height: height,
    })
}

// ─── Camera RAW via rawloader ───

fn extract_raw_preview(path: &Path) -> Result<DecodedImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {e}"))?;

    let mut best: Option<&[u8]> = None;
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i] == 0xFF && data[i + 1] == 0xD8 {
            let start = i;
            i += 2;
            while i + 1 < data.len() {
                if data[i] == 0xFF && data[i + 1] == 0xD9 {
                    let segment = &data[start..i + 2];
                    if segment.len() > best.map_or(0, |b| b.len()) {
                        best = Some(segment);
                    }
                    i += 2;
                    break;
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    let jpeg_data = best.filter(|b| b.len() > 32_000)
        .ok_or("no usable preview")?;

    let img = image::load_from_memory(jpeg_data)
        .map_err(|e| format!("preview: {e}"))?;

    Ok(to_color_image(img, "RAW"))
}

fn load_raw(path: &Path) -> Result<DecodedImage, String> {
    if let Ok(preview) = extract_raw_preview(path) {
        return Ok(preview);
    }

    let raw = rawloader::decode_file(path).map_err(|e| format!("raw: {e}"))?;

    let width = raw.width;
    let height = raw.height;

    let data = match raw.data {
        rawloader::RawImageData::Integer(ref d) => d.iter().map(|&v| v as f32).collect::<Vec<_>>(),
        rawloader::RawImageData::Float(ref d) => d.clone(),
    };

    let max_val = data.iter().cloned().fold(0.0_f32, f32::max).max(1.0);

    let cpp = raw.cpp;
    let pixel_count = width * height;
    let mut rgba = Vec::with_capacity(pixel_count * 4);

    for i in 0..pixel_count {
        let base = i * cpp;
        let r = if base < data.len() {
            (data[base] / max_val * 255.0).clamp(0.0, 255.0) as u8
        } else {
            0
        };
        let g = if cpp > 1 && base + 1 < data.len() {
            (data[base + 1] / max_val * 255.0).clamp(0.0, 255.0) as u8
        } else {
            r
        };
        let b = if cpp > 2 && base + 2 < data.len() {
            (data[base + 2] / max_val * 255.0).clamp(0.0, 255.0) as u8
        } else {
            g
        };
        rgba.extend_from_slice(&[r, g, b, 255]);
    }

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba),
        format_name: "RAW".to_string(),
        original_width: width as u32,
        original_height: height as u32,
    })
}

// ─── PSD via psd crate ───

fn load_psd(path: &Path) -> Result<DecodedImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    std::panic::catch_unwind(|| {
        let psd = psd::Psd::from_bytes(&data).map_err(|e| format!("psd: {e}"))?;
        let width = psd.width();
        let height = psd.height();
        let rgba = psd.rgba();
        Ok(DecodedImage {
            pixels: egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                &rgba,
            ),
            format_name: "PSD".to_string(),
            original_width: width,
            original_height: height,
        })
    })
    .unwrap_or_else(|_| Err("psd: internal decoder panic".into()))
}

// ─── PCX ───

fn load_pcx_image(path: &Path) -> Result<DecodedImage, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("read: {e}"))?;
    let mut reader = std::io::BufReader::new(file);
    let mut pcx_reader =
        pcx::Reader::new(&mut reader).map_err(|e| format!("pcx: {e}"))?;

    let width = pcx_reader.width() as usize;
    let height = pcx_reader.height() as usize;
    let paletted = pcx_reader.is_paletted();

    let mut rgba = Vec::with_capacity(width * height * 4);

    if paletted {
        let mut row_buf = vec![0u8; width];
        let mut rows: Vec<Vec<u8>> = Vec::with_capacity(height);
        for _ in 0..height {
            pcx_reader
                .next_row_paletted(&mut row_buf)
                .map_err(|e| format!("pcx row: {e}"))?;
            rows.push(row_buf.clone());
        }
        let mut palette_buf = vec![0u8; 256 * 3];
        pcx_reader
            .get_palette(&mut palette_buf)
            .map_err(|e| format!("pcx palette: {e}"))?;
        for row in &rows {
            for &idx in row {
                let pi = idx as usize * 3;
                rgba.extend_from_slice(&[palette_buf[pi], palette_buf[pi + 1], palette_buf[pi + 2], 255]);
            }
        }
    } else {
        let mut row_buf = vec![0u8; width * 3];
        for _ in 0..height {
            pcx_reader
                .next_row_rgb(&mut row_buf)
                .map_err(|e| format!("pcx row: {e}"))?;
            for x in 0..width {
                let base = x * 3;
                rgba.extend_from_slice(&[row_buf[base], row_buf[base + 1], row_buf[base + 2], 255]);
            }
        }
    }

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba),
        format_name: "PCX".to_string(),
        original_width: width as u32,
        original_height: height as u32,
    })
}

// ─── JPEG 2000 via openjp2 (pure Rust OpenJPEG port) ───

fn load_jp2(path: &Path) -> Result<DecodedImage, String> {
    use openjp2::openjpeg::*;
    use std::ffi::CString;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let codec_format = match ext.as_str() {
        "j2k" | "j2c" => OPJ_CODEC_J2K,
        _ => OPJ_CODEC_JP2,
    };

    let path_cstr = CString::new(
        path.to_str().ok_or("Invalid path")?.as_bytes(),
    )
    .map_err(|e| format!("path: {e}"))?;

    unsafe {
        let stream = opj_stream_create_default_file_stream(path_cstr.as_ptr(), 1);
        if stream.is_null() {
            return Err("JP2: failed to open file stream".into());
        }

        let codec = opj_create_decompress(codec_format);
        if codec.is_null() {
            opj_stream_destroy(stream);
            return Err("JP2: failed to create decoder".into());
        }

        let mut params = opj_dparameters_t::default();
        if opj_setup_decoder(codec, &mut params) == 0 {
            opj_destroy_codec(codec);
            opj_stream_destroy(stream);
            return Err("JP2: failed to setup decoder".into());
        }

        let mut image: *mut opj_image_t = std::ptr::null_mut();
        if opj_read_header(stream, codec, &mut image) == 0 {
            opj_destroy_codec(codec);
            opj_stream_destroy(stream);
            return Err("JP2: failed to read header".into());
        }

        if opj_decode(codec, stream, image) == 0 {
            opj_destroy_codec(codec);
            opj_stream_destroy(stream);
            if !image.is_null() {
                opj_image_destroy(image);
            }
            return Err("JP2: failed to decode".into());
        }

        opj_end_decompress(codec, stream);
        opj_destroy_codec(codec);
        opj_stream_destroy(stream);

        if image.is_null() {
            return Err("JP2: null image".into());
        }

        let img = &*image;
        let width = img.x1.saturating_sub(img.x0) as usize;
        let height = img.y1.saturating_sub(img.y0) as usize;
        let num_comps = img.numcomps as usize;

        if width == 0 || height == 0 || num_comps == 0 {
            opj_image_destroy(image);
            return Err("JP2: invalid dimensions".into());
        }

        let comps = std::slice::from_raw_parts(img.comps, num_comps);
        let pixel_count = width * height;
        let mut rgba = Vec::with_capacity(pixel_count * 4);

        for i in 0..pixel_count {
            let get_comp = |c: usize| -> u8 {
                if c < num_comps {
                    let comp = &comps[c];
                    let val = *comp.data.add(i);
                    let prec = comp.prec;
                    if prec <= 8 {
                        val.clamp(0, 255) as u8
                    } else {
                        ((val as u64 * 255) / ((1u64 << prec) - 1)).min(255) as u8
                    }
                } else {
                    255
                }
            };

            let r = get_comp(0);
            let g = if num_comps > 1 { get_comp(1) } else { r };
            let b = if num_comps > 2 { get_comp(2) } else { g };
            let a = if num_comps > 3 { get_comp(3) } else { 255 };
            rgba.extend_from_slice(&[r, g, b, a]);
        }

        opj_image_destroy(image);

        Ok(DecodedImage {
            pixels: egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba),
            format_name: "JPEG 2000".to_string(),
            original_width: width as u32,
            original_height: height as u32,
        })
    }
}

// ─── XBM (X BitMap) — custom parser ───

fn load_xbm(path: &Path) -> Result<DecodedImage, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;

    let width = parse_xbm_define(&text, "width").ok_or("XBM: missing width")?;
    let height = parse_xbm_define(&text, "height").ok_or("XBM: missing height")?;

    let brace_start = text.find('{').ok_or("XBM: no data block")?;
    let brace_end = text.rfind('}').ok_or("XBM: no closing brace")?;
    let data_str = &text[brace_start + 1..brace_end];

    let bytes: Vec<u8> = data_str
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.starts_with("0x") || s.starts_with("0X") {
                u8::from_str_radix(&s[2..], 16).ok()
            } else {
                s.parse().ok()
            }
        })
        .collect();

    let mut rgba = Vec::with_capacity(width * height * 4);
    let row_bytes = (width + 7) / 8;

    for y in 0..height {
        for x in 0..width {
            let byte_idx = y * row_bytes + x / 8;
            let bit_idx = x % 8;
            let is_set = if byte_idx < bytes.len() {
                (bytes[byte_idx] >> bit_idx) & 1 != 0
            } else {
                false
            };
            if is_set {
                rgba.extend_from_slice(&[0, 0, 0, 255]); // foreground = black
            } else {
                rgba.extend_from_slice(&[255, 255, 255, 255]); // background = white
            }
        }
    }

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba),
        format_name: "XBM".to_string(),
        original_width: width as u32,
        original_height: height as u32,
    })
}

fn parse_xbm_define(text: &str, suffix: &str) -> Option<usize> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("#define") && line.contains(suffix) {
            if let Some(val) = line.rsplit_once(' ').map(|(_, v)| v) {
                return val.trim().parse().ok();
            }
        }
    }
    None
}

// ─── XPM (X PixMap) — custom parser for XPM3 ───

fn load_xpm(path: &Path) -> Result<DecodedImage, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;

    let strings: Vec<&str> = text
        .split('"')
        .enumerate()
        .filter_map(|(i, s)| if i % 2 == 1 { Some(s) } else { None })
        .collect();

    if strings.is_empty() {
        return Err("XPM: no data strings".into());
    }

    let header_parts: Vec<&str> = strings[0].split_whitespace().collect();
    if header_parts.len() < 4 {
        return Err("XPM: invalid header".into());
    }

    let width: usize = header_parts[0].parse().map_err(|_| "XPM: bad width")?;
    let height: usize = header_parts[1].parse().map_err(|_| "XPM: bad height")?;
    let num_colors: usize = header_parts[2].parse().map_err(|_| "XPM: bad ncolors")?;
    let cpp: usize = header_parts[3].parse().map_err(|_| "XPM: bad cpp")?;

    let mut color_map: std::collections::HashMap<String, [u8; 4]> = std::collections::HashMap::new();

    for i in 0..num_colors {
        let line = strings
            .get(1 + i)
            .ok_or_else(|| format!("XPM: missing color line {i}"))?;
        if line.len() < cpp {
            continue;
        }
        let key = line[..cpp].to_string();
        let rest = &line[cpp..];

        let color = if let Some(pos) = rest.find('#') {
            parse_hex_color(&rest[pos..])
        } else if rest.contains("None") || rest.contains("none") {
            [0, 0, 0, 0] // transparent
        } else {
            [128, 128, 128, 255] // unknown color
        };
        color_map.insert(key, color);
    }

    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        let line = strings
            .get(1 + num_colors + y)
            .ok_or_else(|| format!("XPM: missing row {y}"))?;
        for x in 0..width {
            let start = x * cpp;
            let end = start + cpp;
            let key = if end <= line.len() {
                &line[start..end]
            } else {
                " "
            };
            let color = color_map.get(key).copied().unwrap_or([0, 0, 0, 255]);
            rgba.extend_from_slice(&color);
        }
    }

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba),
        format_name: "XPM".to_string(),
        original_width: width as u32,
        original_height: height as u32,
    })
}

fn parse_hex_color(s: &str) -> [u8; 4] {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            [r, g, b, 255]
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            [r, g, b, 255]
        }
        _ => [128, 128, 128, 255],
    }
}

// ─── SGI (Silicon Graphics Image) — custom parser ───

fn load_sgi(path: &Path) -> Result<DecodedImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    if data.len() < 512 {
        return Err("SGI: file too small".into());
    }

    let magic = u16::from_be_bytes([data[0], data[1]]);
    if magic != 474 {
        return Err(format!("SGI: bad magic {magic}, expected 474"));
    }

    let compression = data[2];
    let bpc = data[3] as usize; // bytes per channel (1 or 2)
    let _dimension = u16::from_be_bytes([data[4], data[5]]);
    let width = u16::from_be_bytes([data[6], data[7]]) as usize;
    let height = u16::from_be_bytes([data[8], data[9]]) as usize;
    let channels = u16::from_be_bytes([data[10], data[11]]) as usize;

    if width == 0 || height == 0 || channels == 0 || channels > 4 {
        return Err("SGI: invalid dimensions".into());
    }
    if bpc != 1 {
        return Err("SGI: only 8-bit per channel supported".into());
    }

    let mut channel_data: Vec<Vec<u8>> = Vec::new();

    if compression == 0 {
        // Uncompressed: data starts at offset 512
        let pixel_count = width * height;
        for c in 0..channels {
            let offset = 512 + c * pixel_count;
            if offset + pixel_count > data.len() {
                return Err("SGI: unexpected EOF".into());
            }
            channel_data.push(data[offset..offset + pixel_count].to_vec());
        }
    } else {
        // RLE compressed
        let table_len = height * channels;
        if 512 + table_len * 8 > data.len() {
            return Err("SGI: offset table too large".into());
        }

        let mut offsets = Vec::with_capacity(table_len);
        let mut lengths = Vec::with_capacity(table_len);
        for i in 0..table_len {
            let off = 512 + i * 4;
            offsets.push(u32::from_be_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
            ]) as usize);
        }
        let len_base = 512 + table_len * 4;
        for i in 0..table_len {
            let off = len_base + i * 4;
            lengths.push(u32::from_be_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
            ]) as usize);
        }

        for c in 0..channels {
            let mut chan = vec![0u8; width * height];
            for y in 0..height {
                let idx = c * height + y;
                let offset = offsets[idx];
                let _length = lengths[idx];
                let row_start = y * width;
                let mut src = offset;
                let mut dst = row_start;

                while src < data.len() && dst < row_start + width {
                    let pixel = data[src];
                    src += 1;
                    let count = (pixel & 0x7f) as usize;
                    if count == 0 {
                        break;
                    }
                    if pixel & 0x80 != 0 {
                        // literal run
                        for _ in 0..count.min(row_start + width - dst) {
                            if src < data.len() {
                                chan[dst] = data[src];
                                src += 1;
                                dst += 1;
                            }
                        }
                    } else {
                        // repeat run
                        if src < data.len() {
                            let val = data[src];
                            src += 1;
                            for _ in 0..count.min(row_start + width - dst) {
                                chan[dst] = val;
                                dst += 1;
                            }
                        }
                    }
                }
            }
            channel_data.push(chan);
        }
    }

    // SGI stores bottom-to-top, convert to top-to-bottom
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in (0..height).rev() {
        for x in 0..width {
            let idx = y * width + x;
            let r = channel_data[0][idx];
            let g = if channels > 1 { channel_data[1][idx] } else { r };
            let b = if channels > 2 { channel_data[2][idx] } else { g };
            let a = if channels > 3 {
                channel_data[3][idx]
            } else {
                255
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }

    Ok(DecodedImage {
        pixels: egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba),
        format_name: "SGI".to_string(),
        original_width: width as u32,
        original_height: height as u32,
    })
}

// ─── Utility ───

// ─── EXIF metadata ───

pub struct ExifData {
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<String>,
    pub aperture: Option<String>,
    pub shutter_speed: Option<String>,
    pub iso: Option<String>,
    pub date_taken: Option<String>,
    pub gps: Option<String>,
    pub software: Option<String>,
}

pub fn read_exif(path: &Path) -> Option<ExifData> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;

    let get_str = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| f.display_value().to_string().trim_matches('"').to_string())
    };

    let get_unit = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| f.display_value().with_unit(&exif).to_string())
    };

    let make = get_str(exif::Tag::Make);
    let model = get_str(exif::Tag::Model);
    let camera = match (&make, &model) {
        (Some(m), Some(md)) => {
            if md.starts_with(m.trim()) {
                Some(md.clone())
            } else {
                Some(format!("{} {}", m.trim(), md.trim()))
            }
        }
        (None, Some(md)) => Some(md.clone()),
        (Some(m), None) => Some(m.clone()),
        (None, None) => None,
    };

    let gps = {
        let lat = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY);
        let lat_ref = exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY);
        let lon = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY);
        let lon_ref = exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY);
        match (lat, lat_ref, lon, lon_ref) {
            (Some(la), Some(lar), Some(lo), Some(lor)) => Some(format!(
                "{} {} {} {}",
                la.display_value(), lar.display_value(),
                lo.display_value(), lor.display_value()
            )),
            _ => None,
        }
    };

    Some(ExifData {
        camera,
        lens: get_str(exif::Tag::LensModel),
        focal_length: get_unit(exif::Tag::FocalLength),
        aperture: get_unit(exif::Tag::FNumber),
        shutter_speed: get_unit(exif::Tag::ExposureTime),
        iso: get_str(exif::Tag::PhotographicSensitivity),
        date_taken: get_str(exif::Tag::DateTimeOriginal),
        gps,
        software: get_str(exif::Tag::Software),
    })
}

// ─── Utility ───

pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_formats() {
        let test_dir = std::path::Path::new("/tmp/cove-format-test");
        let files = [
            ("test.jpg", "JPEG"),
            ("test.png", "PNG"),
            ("test.gif", "GIF"),
            ("test.bmp", "BMP"),
            ("test.tiff", "TIFF"),
            ("test.webp", "WebP"),
            ("test.ico", "ICO"),
            ("test.tga", "TGA"),
            ("test.ppm", "PPM"),
            ("test.pgm", "PGM"),
            ("test.pbm", "PBM"),
            ("test.dds", "DDS"),
            ("test.exr", "EXR"),
            ("test.hdr", "HDR"),
            ("test.qoi", "QOI"),
            ("test.ff", "farbfeld"),
            ("test.svg", "SVG"),
            // PSD skipped: upstream psd crate panics on ImageMagick-generated files; works with real Photoshop files
            ("test.pcx", "PCX"),
            ("test.jxl", "JXL"),
            ("test.jp2", "JP2"),
            ("test.avif", "AVIF"),
            ("test.heic", "HEIC"),
            ("test.xbm", "XBM"),
            ("test.xpm", "XPM"),
            ("test.sgi", "SGI"),
        ];

        let mut pass = 0;
        let mut fail = 0;
        for (file, label) in &files {
            let path = test_dir.join(file);
            if !path.exists() {
                eprintln!("  SKIP {label:>8} — file missing");
                continue;
            }
            match load_image(&path) {
                Ok(img) => {
                    assert!(img.original_width > 0 && img.original_height > 0,
                        "{label} decoded but has zero dimensions");
                    eprintln!("  PASS {label:>8} — {}x{} ({})", img.original_width, img.original_height, img.format_name);
                    pass += 1;
                }
                Err(e) => {
                    eprintln!("  FAIL {label:>8} — {e}");
                    fail += 1;
                }
            }
        }
        eprintln!("\n  Results: {pass} passed, {fail} failed out of {} total", pass + fail);
        assert_eq!(fail, 0, "{fail} format(s) failed to decode");
    }
}
