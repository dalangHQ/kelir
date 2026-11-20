// see: cans/plugins/taint.md#Invariants-and-policy-(taint)
// see: cans/02-plugin-model.md#Facade-blindness
// Shared byte-range label contract: the primitive shape both plugin-taint
// (producer) and plugin-tool-guard (consumer) speak; core holds no
// byte-range semantics, only the neutral carrier type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

// see: cans/plugins/taint.md#Invariants-and-policy-(taint)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaintSet {
    ranges: Vec<ByteRange>,
}

impl TaintSet {
    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn from_ranges(ranges: Vec<ByteRange>) -> Self {
        let mut set = Self::default();
        for range in ranges {
            set.mark(range);
        }
        set
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn mark(&mut self, range: ByteRange) {
        if range.end > range.start && !self.ranges.contains(&range) {
            self.ranges.push(range);
        }
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn propagate(&self, into_len: usize) -> Vec<ByteRange> {
        self.ranges
            .iter()
            .filter_map(|range| {
                let start = range.start.min(into_len);
                let end = range.end.min(into_len);
                if end > start {
                    Some(ByteRange { start, end })
                } else {
                    None
                }
            })
            .collect()
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn overlaps(&self, range: ByteRange) -> bool {
        self.ranges
            .iter()
            .any(|tainted| range.start < tainted.end && tainted.start < range.end)
    }

    // see: cans/plugins/taint.md#Invariants-and-policy-(taint)
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}
