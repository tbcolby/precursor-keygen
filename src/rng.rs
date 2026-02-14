//! TRNG wrapper for Key Ceremony.

extern crate alloc;
use alloc::vec::Vec;

pub struct Rng {
    trng: trng::Trng,
}

impl Rng {
    pub fn new(xns: &xous_names::XousNames) -> Self {
        Self {
            trng: trng::Trng::new(xns).expect("can't connect to TRNG"),
        }
    }

    pub fn u32(&self) -> u32 {
        self.trng.get_u32().unwrap_or(0)
    }

    pub fn range(&self, max: u32) -> u32 {
        if max <= 1 { return 0; }
        let threshold = u32::MAX - (u32::MAX % max);
        loop {
            let val = self.u32();
            if val < threshold {
                return val % max;
            }
        }
    }

    pub fn bytes(&self, n: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(n);
        let mut remaining = n;
        while remaining > 0 {
            let val = self.u32();
            let b = val.to_le_bytes();
            let take = remaining.min(4);
            result.extend_from_slice(&b[..take]);
            remaining -= take;
        }
        result.truncate(n);
        result
    }

    /// Pick a random element from a slice.
    pub fn pick<'a, T>(&self, items: &'a [T]) -> &'a T {
        &items[self.range(items.len() as u32) as usize]
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&self, items: &mut [T]) {
        let n = items.len();
        for i in (1..n).rev() {
            let j = self.range((i + 1) as u32) as usize;
            items.swap(i, j);
        }
    }
}
