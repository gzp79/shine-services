/// A fixed-size bit set backed by packed `u32` words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitSet {
    len: usize,
    words: Box<[u32]>,
}

impl BitSet {
    pub fn new(len: usize) -> Self {
        Self {
            len,
            words: vec![0; len.div_ceil(u32::BITS as usize)].into(),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> bool {
        assert!(
            index < self.len,
            "bit index {index} out of bounds for length {}",
            self.len
        );
        let (word, mask) = self.location(index);
        self.words[word] & mask != 0
    }

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

    pub fn clear(&mut self) {
        self.words.fill(0);
    }

    pub fn packed_words(&self) -> &[u32] {
        &self.words
    }

    pub fn iter_set(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().copied().enumerate().flat_map(|(word_index, word)| {
            let base = word_index * u32::BITS as usize;
            SetBits { word }.map(move |bit| base + bit)
        })
    }

    fn location(&self, index: usize) -> (usize, u32) {
        let word = index / u32::BITS as usize;
        let bit = index % u32::BITS as usize;
        (word, 1 << bit)
    }
}

struct SetBits {
    word: u32,
}

impl Iterator for SetBits {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.word == 0 {
            return None;
        }
        let bit = self.word.trailing_zeros() as usize;
        self.word &= self.word - 1;
        Some(bit)
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
    fn empty_set_has_no_storage() {
        let bits = BitSet::new(0);
        assert!(bits.is_empty());
        assert!(bits.packed_words().is_empty());
        assert_eq!(bits.iter_set().next(), None);
    }
}
