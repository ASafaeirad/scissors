//! Bounded caches for timeline previews decoded away from the UI thread.
use super::*;
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct DecodeKey {
    pub(super) path: String,
    /// `None` selects the first stream of the decoded kind; `Some` is an absolute stream index.
    pub(super) stream: Option<u32>,
    pub(super) duration_millis: u64,
}

impl DecodeKey {
    pub(super) const fn duration(&self) -> f64 {
        std::time::Duration::from_millis(self.duration_millis).as_secs_f64()
    }
}

/// One cache per decoded type, so slow decoders of one kind never starve another.
struct Cache<T> {
    entries: HashMap<DecodeKey, CacheEntry<T>>,
    clock: u64,
}
impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            clock: 0,
        }
    }
}
struct CacheEntry<T> {
    value: Option<Arc<T>>,
    pending: bool,
    accessed: u64,
}
impl<T: 'static> gpui::Global for Cache<T> {}

/// Returns the cached value, or starts decoding it and returns `None` until it is ready.
/// A failed decode is cached as `None` too, so it is not retried every frame.
pub(super) fn request<T: Send + Sync + 'static>(
    path: &str,
    stream: Option<u32>,
    duration: f64,
    capacity: usize,
    decode: fn(&DecodeKey) -> Option<T>,
    cx: &mut Context<ScissorsRoot>,
) -> Option<Arc<T>> {
    if !duration.is_finite() || duration <= 0.0 {
        return None;
    }
    let key = DecodeKey {
        path: path.into(),
        stream,
        duration_millis: crate::numeric::rounded_f64_to_u64(duration * 1000.0),
    };
    if cx.try_global::<Cache<T>>().is_none() {
        cx.set_global(Cache::<T>::default());
    }
    let cache = cx.global_mut::<Cache<T>>();
    cache.clock += 1;
    if let Some(entry) = cache.entries.get_mut(&key) {
        entry.accessed = cache.clock;
        return entry.value.clone();
    }
    // Limit concurrent decoders. Completion triggers a redraw to request queued sources.
    if cache.entries.values().filter(|entry| entry.pending).count() >= 2 {
        return None;
    }
    if cache.entries.len() >= capacity {
        let oldest = cache
            .entries
            .iter()
            .filter(|(_, entry)| !entry.pending)
            .min_by_key(|(_, entry)| entry.accessed)
            .map(|(key, _)| key.clone());
        if let Some(oldest) = oldest {
            cache.entries.remove(&oldest);
        }
    }
    cache.entries.insert(
        key.clone(),
        CacheEntry {
            value: None,
            pending: true,
            accessed: cache.clock,
        },
    );
    cx.spawn(async move |this, cx| {
        let decode_key = key.clone();
        let result = cx
            .background_spawn(async move { decode(&decode_key).map(Arc::new) })
            .await;
        this.update(cx, |_, cx| {
            if let Some(entry) = cx.global_mut::<Cache<T>>().entries.get_mut(&key) {
                entry.value = result;
                entry.pending = false;
            }
            cx.notify();
        })
        .ok();
    })
    .detach();
    None
}

/// Maps a time along concatenated source ranges back to the source time it shows.
pub(super) fn source_time(ranges: &[(f64, f64)], mut offset: f64) -> f64 {
    for &(start, end) in ranges {
        let length = end - start;
        if offset < length {
            return start + offset.max(0.0);
        }
        offset -= length;
    }
    ranges.last().map_or(0.0, |&(_, end)| end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_time_skips_the_gaps_between_ranges() {
        let ranges = [(1.0, 3.0), (5.0, 6.0)];
        assert!((source_time(&ranges, 0.0) - 1.0).abs() < f64::EPSILON);
        assert!((source_time(&ranges, 1.5) - 2.5).abs() < f64::EPSILON);
        assert!((source_time(&ranges, 2.5) - 5.5).abs() < f64::EPSILON);
        assert!((source_time(&ranges, 9.0) - 6.0).abs() < f64::EPSILON);
        assert!(source_time(&[], 1.0).abs() < f64::EPSILON);
    }
}
