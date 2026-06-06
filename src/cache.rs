use crate::decoder::DecodedImage;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub enum CachedImage {
    Static(DecodedImage),
    Animated(Vec<(egui::ColorImage, u32)>),
}

struct Entry {
    path: PathBuf,
    data: CachedImage,
}

pub struct ImageCache {
    entries: VecDeque<Entry>,
    max_entries: usize,
}

impl ImageCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
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
        self.entries.retain(|e| e.path != path);
        while self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry { path, data });
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|e| e.path == path)
    }
}
