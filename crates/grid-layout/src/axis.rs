use std::ops::Range;

/// Prefix sums that can be updated in logarithmic time.
#[derive(Clone, Debug)]
struct Sums {
    tree: Vec<f64>,
}

impl Sums {
    fn new(sizes: &[f32]) -> Self {
        let mut tree = vec![0.0; sizes.len() + 1];
        for (index, size) in sizes.iter().enumerate() {
            let at = index + 1;
            tree[at] += f64::from(*size);
            let parent = at + at.isolate_lowest_one();
            if parent < tree.len() {
                tree[parent] += tree[at];
            }
        }
        Self { tree }
    }

    fn add(&mut self, index: usize, delta: f64) {
        let mut at = index + 1;
        while at < self.tree.len() {
            self.tree[at] += delta;
            at += at.isolate_lowest_one();
        }
    }

    /// The sum of the first `count` sizes.
    fn prefix(&self, count: usize) -> f64 {
        let mut at = count.min(self.tree.len() - 1);
        let mut sum = 0.0;
        while at > 0 {
            sum += self.tree[at];
            at -= at.isolate_lowest_one();
        }
        sum
    }

    /// The index of the item that contains `offset`.
    fn find(&self, offset: f64) -> usize {
        let mut at = 0;
        let mut rest = offset;
        let mut step = (self.tree.len() - 1).next_power_of_two();
        while step > 0 {
            let next = at + step;
            if next < self.tree.len() && self.tree[next] <= rest {
                at = next;
                rest -= self.tree[next];
            }
            step /= 2;
        }
        at
    }
}

/// One axis of a grid: the sizes of its rows or of its columns.
///
/// A uniform axis answers everything by arithmetic and holds no
/// per-item state, so a million equal rows cost nothing. A variable
/// axis keeps prefix sums.
#[derive(Clone, Debug)]
pub enum Axis {
    Uniform { count: usize, size: f32 },
    Variable { sizes: Vec<f32>, sums: SumsBox },
}

/// Opaque prefix sums of a variable axis.
#[derive(Clone, Debug)]
pub struct SumsBox(Sums);

impl Axis {
    pub fn uniform(count: usize, size: f32) -> Self {
        Self::Uniform { count, size: size.max(0.0) }
    }

    pub fn variable(sizes: Vec<f32>) -> Self {
        let sizes: Vec<f32> = sizes.into_iter().map(|size| size.max(0.0)).collect();
        let sums = SumsBox(Sums::new(&sizes));
        Self::Variable { sizes, sums }
    }

    pub fn count(&self) -> usize {
        match self {
            Self::Uniform { count, .. } => *count,
            Self::Variable { sizes, .. } => sizes.len(),
        }
    }

    pub fn total(&self) -> f64 {
        self.offset(self.count())
    }

    pub fn size(&self, index: usize) -> f32 {
        match self {
            Self::Uniform { count, size } => {
                if index < *count {
                    *size
                } else {
                    0.0
                }
            }
            Self::Variable { sizes, .. } => sizes.get(index).copied().unwrap_or(0.0),
        }
    }

    /// Where item `index` starts. `index == count` is the end.
    pub fn offset(&self, index: usize) -> f64 {
        match self {
            Self::Uniform { count, size } => index.min(*count) as f64 * f64::from(*size),
            Self::Variable { sums, .. } => sums.0.prefix(index),
        }
    }

    /// The item that contains `offset`, clamped to the axis. `None`
    /// for an empty axis.
    pub fn index_at(&self, offset: f64) -> Option<usize> {
        let count = self.count();
        if count == 0 {
            return None;
        }
        let offset = offset.max(0.0);
        let index = match self {
            Self::Uniform { size, .. } if *size > 0.0 => (offset / f64::from(*size)) as usize,
            Self::Uniform { .. } => 0,
            Self::Variable { sums, .. } => sums.0.find(offset),
        };
        Some(index.min(count - 1))
    }

    /// The items that intersect `[from, to)`.
    pub fn range(&self, from: f64, to: f64) -> Range<usize> {
        if to <= from {
            return 0..0;
        }
        match (self.index_at(from), self.index_at(to)) {
            (Some(first), Some(last)) => {
                let end = if self.offset(last) < to { last + 1 } else { last };
                first..end.max(first)
            }
            _ => 0..0,
        }
    }

    /// Resizes one item. A uniform axis becomes variable.
    pub fn set_size(&mut self, index: usize, size: f32) {
        let size = size.max(0.0);
        if index >= self.count() {
            return;
        }
        if let Self::Uniform { count, size: old } = *self {
            if old == size {
                return;
            }
            *self = Self::variable(vec![old; count]);
        }
        if let Self::Variable { sizes, sums } = self {
            sums.0.add(index, f64::from(size) - f64::from(sizes[index]));
            sizes[index] = size;
        }
    }
}
