/// A fixed-size bit set backed by packed `u32` words. `len` is always positive, so a `BitSet`
/// is never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitSet {
    len: usize,
    words: Box<[u32]>,
}

impl BitSet {
    #[inline]
    pub fn new(len: usize) -> Self {
        assert!(len > 0, "BitSet length must be positive");
        Self {
            len,
            words: vec![0; len.div_ceil(u32::BITS as usize)].into(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn get(&self, index: usize) -> bool {
        assert!(
            index < self.len,
            "bit index {index} out of bounds for length {}",
            self.len
        );
        let (word, mask) = self.location(index);
        self.words[word] & mask != 0
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: bool) {
        assert!(
            index < self.len,
            "bit index {index} out of bounds for length {}",
            self.len
        );
        let (word, mask) = self.location(index);
        if value {
            self.words[word] |= mask;
        } else {
            self.words[word] &= !mask;
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.words.fill(0);
    }

    #[inline]
    pub fn fill(&mut self, value: bool) {
        let mask = if value { !0 } else { 0 };
        self.words.fill(mask);
        self.mask_trailing_bits();
    }

    #[inline]
    pub fn packed_words(&self) -> &[u32] {
        &self.words
    }

    /// `true` if at least one bit is set.
    #[inline]
    pub fn any(&self) -> bool {
        self.words.iter().any(|&word| word != 0)
    }

    /// `true` if every bit in the domain is set.
    #[inline]
    pub fn all(&self) -> bool {
        let (last, rest) = self.words.split_last().expect("BitSet length is always positive");
        if rest.iter().any(|&word| word != u32::MAX) {
            return false;
        }
        let rem = self.len % u32::BITS as usize;
        let mask = if rem == 0 { u32::MAX } else { (1 << rem) - 1 };
        *last == mask
    }

    #[inline]
    pub fn iter_set(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().copied().enumerate().flat_map(|(word_index, word)| {
            let base = word_index * u32::BITS as usize;
            SetBits { word }.map(move |bit| base + bit)
        })
    }

    #[inline]
    fn location(&self, index: usize) -> (usize, u32) {
        let word = index / u32::BITS as usize;
        let bit = index % u32::BITS as usize;
        (word, 1 << bit)
    }
    /// Clears the padding bits beyond `len` in the last word, restoring the invariant that
    /// bits outside the domain are always 0 (relied on by `iter_set` and `packed_words`).
    #[inline]
    fn mask_trailing_bits(&mut self) {
        let rem = self.len % u32::BITS as usize;
        if rem != 0 {
            if let Some(last) = self.words.last_mut() {
                *last &= (1 << rem) - 1;
            }
        }
    }
}

/// Iterator over the set bit positions (0-based, LSB first) of a single packed word.
struct SetBits {
    word: u32,
}

impl Iterator for SetBits {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.word == 0 {
            return None;
        }
        let bit = self.word.trailing_zeros() as usize;
        self.word &= self.word - 1;
        Some(bit)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.word.count_ones() as usize;
        (remaining, Some(remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    #[test]
    fn fixed_size_storage_and_access() {
        let mut bits = BitSet::new(65);
        assert_eq!(bits.len(), 65);
        assert_eq!(bits.packed_words(), &[0, 0, 0]);

        for index in [0, 31, 32, 64] {
            bits.set(index, true);
            assert!(bits.get(index));
        }
        bits.set(31, false);

        assert_eq!(bits.packed_words(), &[1, 1, 1]);
        assert_eq!(bits.iter_set().collect::<Vec<_>>(), vec![0, 32, 64]);

        bits.clear();
        assert_eq!(bits.len(), 65);
        assert_eq!(bits.packed_words(), &[0, 0, 0]);
        assert_eq!(bits.iter_set().next(), None);
    }

    #[test]
    fn fill_does_not_set_bits_outside_domain() {
        let mut bits = BitSet::new(65);
        bits.fill(true);

        assert_eq!(bits.packed_words(), &[u32::MAX, u32::MAX, 1]);
        assert_eq!(bits.iter_set().collect::<Vec<_>>(), (0..65).collect::<Vec<_>>());

        bits.fill(false);
        assert_eq!(bits.packed_words(), &[0, 0, 0]);
        assert_eq!(bits.iter_set().next(), None);
    }

    #[test]
    fn any_and_all_track_domain_bits_only() {
        let mut bits = BitSet::new(65);
        assert!(!bits.any());
        assert!(!bits.all());

        bits.set(0, true);
        assert!(bits.any());
        assert!(!bits.all());

        bits.fill(true);
        assert!(bits.any());
        assert!(bits.all());

        bits.set(64, false);
        assert!(bits.any());
        assert!(!bits.all());

        bits.fill(false);
        assert!(!bits.any());
        assert!(!bits.all());
    }
}
