use std::hash::Hash;

use lru::LruCache;

/// Trait for values whose memory footprint can be estimated.
pub trait ByteSize {
    /// Approximate heap + inline bytes occupied by this value.
    fn byte_size(&self) -> u64;
}

/// An LRU cache bounded by total byte weight rather than entry count.
///
/// The underlying `lru::LruCache` is unbounded in count; eviction is driven by
/// the tracked byte budget. A single entry larger than the cap is still
/// admitted (so callers never fail to cache), but it immediately evicts every
/// other entry.
pub struct ByteLru<K, V> {
    inner: LruCache<K, V>,
    cap: u64,
    used: u64,
}

impl<K: Hash + Eq, V: ByteSize> ByteLru<K, V> {
    pub fn new(cap: u64) -> Self {
        // Unbounded in count; byte budget drives eviction.
        Self {
            inner: LruCache::unbounded(),
            cap,
            used: 0,
        }
    }

    pub fn get<'a, Q>(&'a mut self, k: &Q) -> Option<&'a V>
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.get(k)
    }

    pub fn contains<Q>(&self, k: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.contains(k)
    }

    /// Inserts a value, evicting LRU entries until the budget fits.
    ///
    /// Returns the previous value if the key was already present.
    pub fn put(&mut self, k: K, v: V) -> Option<V> {
        let size = v.byte_size();
        if let Some(old) = self.inner.pop(&k) {
            self.used = self.used.saturating_sub(old.byte_size());
        }
        while self.used.saturating_add(size) > self.cap {
            match self.inner.pop_lru() {
                Some((_, evicted)) => {
                    self.used = self.used.saturating_sub(evicted.byte_size());
                }
                None => break,
            }
        }
        self.used = self.used.saturating_add(size);
        self.inner.put(k, v)
    }

    pub fn clear(&mut self) {
        self.inner.clear();
        self.used = 0;
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }

    #[cfg(test)]
    pub(crate) fn used_bytes(&self) -> u64 {
        self.used
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl ByteSize for u64 {
        fn byte_size(&self) -> u64 {
            *self
        }
    }

    #[test]
    fn evicts_lru_to_stay_within_budget() {
        let mut cache = ByteLru::new(10);
        cache.put(1, 4u64);
        cache.put(2, 4u64);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.used_bytes(), 8);

        // This insert exceeds the cap; key 1 (LRU) must be evicted.
        cache.put(3, 5u64);
        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(&1));
        assert!(cache.contains(&2));
        assert!(cache.contains(&3));
        assert_eq!(cache.used_bytes(), 9);
    }

    #[test]
    fn replaces_existing_key_without_double_counting() {
        let mut cache = ByteLru::new(10);
        cache.put(1, 4u64);
        cache.put(1, 6u64);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.used_bytes(), 6);
    }

    #[test]
    fn admits_an_entry_larger_than_the_cap() {
        let mut cache = ByteLru::new(5);
        cache.put(1, 4u64);
        cache.put(2, 10u64);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&2));
        assert_eq!(cache.used_bytes(), 10);
    }

    #[test]
    fn clear_resets_the_budget() {
        let mut cache = ByteLru::new(10);
        cache.put(1, 4u64);
        cache.put(2, 4u64);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.used_bytes(), 0);
    }
}
