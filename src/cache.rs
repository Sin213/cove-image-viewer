use crate::decoder::DecodedImage;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone)]
pub enum CachedImage {
    Static(DecodedImage),
    Animated(Vec<(egui::ColorImage, u32)>),
}

impl CachedImage {
    fn byte_size(&self) -> usize {
        match self {
            CachedImage::Static(d) => d.pixels.size[0] * d.pixels.size[1] * 4,
            CachedImage::Animated(frames) => {
                frames.iter().map(|(img, _)| img.size[0] * img.size[1] * 4).sum()
            }
        }
    }
}

struct Entry {
    path: PathBuf,
    data: CachedImage,
    byte_size: usize,
}

pub struct ImageCache {
    entries: VecDeque<Entry>,
    current_bytes: usize,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            current_bytes: 0,
        }
    }

    pub fn get(&mut self, path: &Path) -> Option<CachedImage> {
        if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
            let entry = self.entries.remove(pos).unwrap();
            let data = entry.data.clone();
            self.entries.push_back(entry);
            Some(data)
        } else {
            None
        }
    }

    pub fn put(&mut self, path: PathBuf, data: CachedImage) {
        if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
            let old = self.entries.remove(pos).unwrap();
            self.current_bytes -= old.byte_size;
        }
        let byte_size = data.byte_size();
        while self.current_bytes + byte_size > MAX_BYTES {
            if let Some(evicted) = self.entries.pop_front() {
                self.current_bytes -= evicted.byte_size;
            } else {
                break;
            }
        }
        self.current_bytes += byte_size;
        self.entries.push_back(Entry { path, data, byte_size });
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|e| e.path == path)
    }
}
