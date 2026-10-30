//! Murmur2 fingerprinting shared by every content provider.

/// Bytes that Murmur2 skips so a file hashes the same regardless of how its
/// whitespace was rewritten in transit.
pub(crate) fn is_fingerprint_whitespace(byte: u8) -> bool {
    matches!(byte, 9 | 10 | 13 | 32)
}

pub(crate) struct Murmur2Hasher {
    pub(crate) hash: u32,
    pub(crate) tail: [u8; 4],
    pub(crate) tail_len: usize,
}

impl Murmur2Hasher {
    pub(crate) fn new(normalized_size: u32) -> Self {
        Self {
            hash: 1 ^ normalized_size,
            tail: [0; 4],
            tail_len: 0,
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if is_fingerprint_whitespace(byte) {
                continue;
            }
            self.tail[self.tail_len] = byte;
            self.tail_len += 1;
            if self.tail_len == self.tail.len() {
                self.mix_word(u32::from_le_bytes(self.tail));
                self.tail_len = 0;
            }
        }
    }

    fn mix_word(&mut self, mut value: u32) {
        const M: u32 = 0x5bd1e995;
        const R: u32 = 24;
        value = value.wrapping_mul(M);
        value ^= value >> R;
        value = value.wrapping_mul(M);
        self.hash = self.hash.wrapping_mul(M);
        self.hash ^= value;
    }

    pub(crate) fn finish(mut self) -> u32 {
        const M: u32 = 0x5bd1e995;
        match self.tail[..self.tail_len] {
            [a, b, c] => {
                self.hash ^= (c as u32) << 16;
                self.hash ^= (b as u32) << 8;
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [a, b] => {
                self.hash ^= (b as u32) << 8;
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [a] => {
                self.hash ^= a as u32;
                self.hash = self.hash.wrapping_mul(M);
            }
            [] => {}
            _ => unreachable!(),
        }
        self.hash ^= self.hash >> 13;
        self.hash = self.hash.wrapping_mul(M);
        self.hash ^= self.hash >> 15;
        self.hash
    }
}

pub(crate) fn compute_fingerprint(data: &[u8]) -> u32 {
    let normalized_size = data
        .iter()
        .filter(|byte| !is_fingerprint_whitespace(**byte))
        .count() as u32;
    let mut hasher = Murmur2Hasher::new(normalized_size);
    hasher.update(data);
    hasher.finish()
}
