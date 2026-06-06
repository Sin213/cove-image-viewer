use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq)]
pub enum SortMode {
    Name,
    DateModified,
    Size,
    Extension,
}

const SUPPORTED_EXTENSIONS: &[&str] = &[
    // Tier 1: image crate (JPEG, PNG, GIF, BMP, TIFF, WebP, ICO, QOI, EXR, HDR, TGA, DDS, PNM, farbfeld)
    "jpg", "jpeg", "jpe", "jfif", "png", "apng", "gif", "bmp", "dib", "tif", "tiff", "webp",
    "ico", "cur", "ppm", "pgm", "pbm", "pam", "pnm", "dds", "tga", "ff", "exr", "qoi", "hdr",
    // Tier 2: extended pure-Rust decoders
    "svg", "svgz", "jxl", "psd", "psb", "pcx",
    // Camera RAW (via rawloader)
    "cr2", "nef", "arw", "dng", "orf", "rw2", "raf", "pef", "srw", "mrw", "erf", "kdc", "dcr",
    "raw", "3fr", "ari", "bay", "cap", "crw", "dcs", "drf", "fff", "iiq", "mos", "mef", "nrw",
    "ptx", "pxn", "r3d", "rwl", "rwz", "sr2", "srf", "x3f",
    // HEIF/HEIC/AVIF (via libheif)
    "heic", "heif", "avif", "hif",
    // JPEG 2000 (via jp2k/openjpeg)
    "jp2", "j2k", "j2c", "jpx", "jpf", "jpm",
    // Custom parsers
    "xbm", "xpm", "sgi", "rgb", "rgba", "bw", "int", "inta",
];

pub struct Browser {
    pub files: Vec<PathBuf>,
    pub index: usize,
    pub directory: Option<PathBuf>,
    pub sort_mode: SortMode,
}

impl Browser {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            index: 0,
            directory: None,
            sort_mode: SortMode::Name,
        }
    }

    pub fn open_path(&mut self, path: &Path) {
        if path.is_dir() {
            self.scan_directory(path);
        } else if path.is_file() {
            let dir = path.parent().unwrap_or(Path::new("."));
            self.scan_directory(dir);
            if let Some(pos) = self.files.iter().position(|f| f == path) {
                self.index = pos;
            }
        }
    }

    fn scan_directory(&mut self, dir: &Path) {
        self.directory = Some(dir.to_path_buf());
        self.files.clear();
        self.index = 0;

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && Self::is_supported(&path) {
                    self.files.push(path);
                }
            }
        }

        self.apply_sort();
    }

    fn is_supported(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    pub fn current(&self) -> Option<&Path> {
        self.files.get(self.index).map(|p| p.as_path())
    }

    pub fn next(&mut self) -> bool {
        if self.index + 1 < self.files.len() {
            self.index += 1;
            true
        } else if !self.files.is_empty() {
            self.index = 0;
            true
        } else {
            false
        }
    }

    pub fn prev(&mut self) -> bool {
        if self.index > 0 {
            self.index -= 1;
            true
        } else if !self.files.is_empty() {
            self.index = self.files.len() - 1;
            true
        } else {
            false
        }
    }

    pub fn first(&mut self) -> bool {
        if !self.files.is_empty() && self.index != 0 {
            self.index = 0;
            true
        } else {
            false
        }
    }

    pub fn last(&mut self) -> bool {
        if !self.files.is_empty() {
            let last = self.files.len() - 1;
            if self.index != last {
                self.index = last;
                return true;
            }
        }
        false
    }

    pub fn position_label(&self) -> String {
        if self.files.is_empty() {
            String::new()
        } else {
            format!("{}/{}", self.index + 1, self.files.len())
        }
    }

    pub fn sort_by(&mut self, mode: SortMode) {
        let current_file = self.current().map(|p| p.to_path_buf());
        self.sort_mode = mode;
        self.apply_sort();
        if let Some(cf) = current_file {
            if let Some(pos) = self.files.iter().position(|f| f == &cf) {
                self.index = pos;
            }
        }
    }

    fn apply_sort(&mut self) {
        match self.sort_mode {
            SortMode::Name => self.files.sort_by(|a, b| natord_compare(a, b)),
            SortMode::DateModified => self.files.sort_by(|a, b| {
                let a_time = std::fs::metadata(a).and_then(|m| m.modified()).ok();
                let b_time = std::fs::metadata(b).and_then(|m| m.modified()).ok();
                a_time.cmp(&b_time)
            }),
            SortMode::Size => self.files.sort_by(|a, b| {
                let a_size = std::fs::metadata(a).map(|m| m.len()).unwrap_or(0);
                let b_size = std::fs::metadata(b).map(|m| m.len()).unwrap_or(0);
                a_size.cmp(&b_size)
            }),
            SortMode::Extension => self.files.sort_by(|a, b| {
                let a_ext = a.extension().and_then(|e| e.to_str()).unwrap_or("");
                let b_ext = b.extension().and_then(|e| e.to_str()).unwrap_or("");
                a_ext.to_lowercase().cmp(&b_ext.to_lowercase())
            }),
        }
    }

    pub fn remove_current(&mut self) {
        if self.files.is_empty() {
            return;
        }
        self.files.remove(self.index);
        if self.index >= self.files.len() && !self.files.is_empty() {
            self.index = self.files.len() - 1;
        }
    }
}

fn natord_compare(a: &Path, b: &Path) -> std::cmp::Ordering {
    let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let mut a_chars = a_name.chars().peekable();
    let mut b_chars = b_name.chars().peekable();

    loop {
        match (a_chars.peek(), b_chars.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(&ac), Some(&bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    let a_num = collect_number(&mut a_chars);
                    let b_num = collect_number(&mut b_chars);
                    match a_num.cmp(&b_num) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                } else {
                    let ac_lower = ac.to_lowercase().next().unwrap_or(ac);
                    let bc_lower = bc.to_lowercase().next().unwrap_or(bc);
                    match ac_lower.cmp(&bc_lower) {
                        std::cmp::Ordering::Equal => {
                            a_chars.next();
                            b_chars.next();
                        }
                        other => return other,
                    }
                }
            }
        }
    }
}

fn collect_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> u64 {
    let mut n: u64 = 0;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            n = n.saturating_mul(10).saturating_add(c as u64 - '0' as u64);
            chars.next();
        } else {
            break;
        }
    }
    n
}
