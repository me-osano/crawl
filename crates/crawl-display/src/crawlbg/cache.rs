//! Image cache with LRU eviction.
//!
//! Provides bounded caching of decoded images to avoid redundant
//! disk I/O and decoding. Uses LRU eviction when capacity is reached.

use lru::LruCache;
use image::DynamicImage;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Mutex;
use anyhow::Context;
use tracing::debug;

/// Default cache capacity (number of images).
const DEFAULT_CAPACITY: usize = 20;

/// Thread-safe LRU cache for wallpaper images.
pub struct ImageCache {
    inner: Mutex<LruCache<String, DynamicImage>>,
    capacity: NonZeroUsize,
}

impl ImageCache {
    /// Create a new cache with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a new cache with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap_or_else(|| unsafe { NonZeroUsize::new_unchecked(1) });
        Self {
            inner: Mutex::new(LruCache::new(cap)),
            capacity: cap,
        }
    }

    /// Get a cached image by path. Returns `None` if not cached.
    pub fn get(&self, path: &str) -> Option<DynamicImage> {
        let mut cache = self.inner.lock().unwrap();
        cache.get(&path.to_string()).cloned()
    }

    /// Insert an image into the cache. Evicts LRU entry if at capacity.
    pub fn put(&self, path: String, image: DynamicImage) {
        let mut cache = self.inner.lock().unwrap();
        if cache.len() >= self.capacity.get() && !cache.contains(&path) {
            if let Some((evicted_key, _)) = cache.pop_lru() {
                debug!("cache: evicted LRU entry: {}", evicted_key);
            }
        }
        cache.put(path, image);
    }

    /// Check if an image is cached.
    pub fn contains(&self, path: &str) -> bool {
        let cache = self.inner.lock().unwrap();
        cache.contains(&path.to_string())
    }

    /// Load an image from cache or disk.
    pub fn get_or_load(&self, path: &Path) -> anyhow::Result<DynamicImage> {
        let path_str = path.to_string_lossy().to_string();
        
        // Check cache first
        if let Some(img) = self.get(&path_str) {
            debug!("cache hit: {}", path.display());
            return Ok(img);
        }

        // Load from disk
        debug!("cache miss, loading: {}", path.display());
        let img = image::open(path)
            .with_context(|| format!("load image {}", path.display()))?;
        
        self.put(path_str.clone(), img.clone());
        Ok(img)
    }

    /// Preload an image into the cache.
    pub fn preload(&self, path: &Path) -> anyhow::Result<()> {
        let path_str = path.to_string_lossy().to_string();
        if !self.contains(&path_str) {
            let img = image::open(path)
                .with_context(|| format!("preload image {}", path.display()))?;
            self.put(path_str.clone(), img);
            debug!("preloaded: {}", path.display());
        }
        Ok(())
    }

    /// Clear all cached images.
    pub fn clear(&self) {
        let mut cache = self.inner.lock().unwrap();
        cache.clear();
        debug!("cache cleared");
    }

    /// Remove a specific entry from the cache.
    pub fn remove(&self, path: &str) {
        let mut cache = self.inner.lock().unwrap();
        cache.pop(path);
    }

    /// Get current cache size.
    pub fn len(&self) -> usize {
        let cache = self.inner.lock().unwrap();
        cache.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}
