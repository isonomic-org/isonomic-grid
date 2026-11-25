/// One bit per row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    words: Vec<u64>,
    len: usize,
}

impl Bitmap {
    pub fn new(len: usize, value: bool) -> Self {
        let mut out = Self { words: vec![if value { u64::MAX } else { 0 }; len.div_ceil(64)], len };
        out.trim();
        out
    }

    pub fn from_fn(len: usize, mut test: impl FnMut(usize) -> bool) -> Self {
        let mut out = Self::new(len, false);
        for (index, word) in out.words.iter_mut().enumerate() {
            let base = index * 64;
            let mut bits = 0u64;
            for bit in 0..64.min(len - base) {
                bits |= u64::from(test(base + bit)) << bit;
            }
            *word = bits;
        }
        out
    }

    /// Clears the bits past the end.
    fn trim(&mut self) {
        let used = self.len % 64;
        if used != 0
            && let Some(last) = self.words.last_mut()
        {
            *last &= (1u64 << used) - 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> bool {
        index < self.len && self.words[index / 64] >> (index % 64) & 1 == 1
    }

    pub fn and(&mut self, other: &Bitmap) {
        for (word, theirs) in self.words.iter_mut().zip(&other.words) {
            *word &= theirs;
        }
    }

    pub fn or(&mut self, other: &Bitmap) {
        for (word, theirs) in self.words.iter_mut().zip(&other.words) {
            *word |= theirs;
        }
    }

    pub fn not(&mut self) {
        for word in &mut self.words {
            *word = !*word;
        }
        self.trim();
    }

    pub fn count(&self) -> usize {
        self.words.iter().map(|word| word.count_ones() as usize).sum()
    }

    /// The set positions, ascending.
    pub fn ones(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.count());
        for (index, word) in self.words.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                out.push((index * 64) as u32 + bits.trailing_zeros());
                bits &= bits - 1;
            }
        }
        out
    }
}
