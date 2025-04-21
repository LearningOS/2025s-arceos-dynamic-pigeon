#![no_std]

use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use arceos_api::modules::axhal;
struct SimpleHasher {
    state: u64,
    random: u64,
}

impl SimpleHasher {
    
    fn new(random: u64) -> Self {
        SimpleHasher { state: 0, random }
    }
}

impl Hasher for SimpleHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state = self.state.wrapping_mul(31).wrapping_add(*byte as u64);
        }
    }
}

pub struct HashMap<K, V> {
    buckets: Vec<Vec<(K, V)>>,
    count: usize,
    _marker: PhantomData<(K, V)>,
    hasher: u64,
}

impl<K, V> HashMap<K, V>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        let mut buckets = Vec::with_capacity(16);
        for _ in 0..16 {
            buckets.push(Vec::new());
        }
        HashMap {
            buckets,
            count: 0,
            _marker: PhantomData,
            hasher: axhal::misc::random() as u64,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut buckets = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buckets.push(Vec::new());
        }
        HashMap {
            buckets,
            count: 0,
            _marker: PhantomData,
            hasher: axhal::misc::random() as u64,
        }
    }

    fn resize(&mut self) {
        let new_size = self.buckets.len() * 2;
        let mut new_buckets = Vec::with_capacity(new_size);
        for _ in 0..new_size {
            new_buckets.push(Vec::new());
        }
        core::mem::swap(&mut self.buckets, &mut new_buckets);
        for bucket in new_buckets {
            for (key, value) in bucket {
                let mut hasher = SimpleHasher::new(self.hasher);
                key.hash(&mut hasher);
                let index = (hasher.finish() % new_size as u64) as usize;
                self.buckets[index].push((key, value));
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.count > self.buckets.len() * 2 {
            self.resize();
        }
        let mut hasher = SimpleHasher::new(self.hasher);
        key.hash(&mut hasher);
        let index = (hasher.finish() % self.buckets.len() as u64) as usize;
        for &mut (ref existing_key, ref mut existing_value) in &mut self.buckets[index] {
            if existing_key == &key {
                *existing_value = value;
                return;
            }
        }
        self.buckets[index].push((key, value));
        self.count += 1;
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let mut hasher = SimpleHasher::new(self.hasher);
        key.hash(&mut hasher);
        let index = (hasher.finish() % self.buckets.len() as u64) as usize;
        for (existing_key, value) in &self.buckets[index] {
            if existing_key == key {
                return Some(value);
            }
        }
        None
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.buckets.iter().flat_map(|bucket| bucket.iter().map(|(k, v)| (k, v)))
    }
}