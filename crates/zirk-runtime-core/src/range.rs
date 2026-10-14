//! Platform-neutral value semantics for Zirk `Range<T>`.
//!
//! The native runtime owns collector handles; this module owns only the
//! arithmetic-sequence rules shared by native code and the interpreter.

use crate::duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: i64,
    pub end: i64,
    pub step: i64,
    pub inclusive: bool,
}

impl Range {
    pub const fn new(start: i64, end: i64, step: i64, inclusive: bool) -> Self {
        Self {
            start,
            end,
            step,
            inclusive,
        }
    }

    pub fn len(self) -> Option<i64> {
        let step = self.step as i128;
        if step == 0 {
            return Some(0);
        }
        let distance = if step > 0 {
            self.end as i128 - self.start as i128
        } else {
            self.start as i128 - self.end as i128
        } + i128::from(self.inclusive);
        if distance <= 0 {
            return Some(0);
        }
        let count =
            distance.div_euclid(step.abs()) + i128::from(distance.rem_euclid(step.abs()) != 0);
        i64::try_from(count).ok()
    }

    /// An invalid, empty, or finite zero-length range is empty. `None`
    /// represents a range too large for an `i64` length, not an empty range.
    pub fn is_empty(self) -> bool {
        self.len() == Some(0)
    }

    pub fn element_at(self, index: i64) -> i64 {
        (self.start as i128 + index as i128 * self.step as i128)
            .clamp(i64::MIN as i128, i64::MAX as i128) as i64
    }

    pub fn slice(self, start: i64, end: i64, step: i64) -> Self {
        const MISSING: i64 = i64::MIN;
        let step = if step == MISSING { 1 } else { step };
        if step == 0 || self.len() == Some(0) {
            return Self::new(0, 0, 1, false);
        }
        let n = self.len().unwrap_or(i64::MAX);
        let forward = |bound: i64| {
            if bound < 0 {
                (n + bound).max(0)
            } else {
                bound.min(n)
            }
        };
        let (first, last) = if step > 0 {
            let lo = if start == MISSING { 0 } else { forward(start) };
            let hi = if end == MISSING { n } else { forward(end) };
            if lo >= hi {
                return Self::new(0, 0, 1, false);
            }
            (lo, lo + (hi - 1 - lo) / step * step)
        } else {
            let lo = if start == MISSING {
                n - 1
            } else {
                (if start < 0 { n + start } else { start }).clamp(-1, n - 1)
            };
            let hi = if end == MISSING {
                -1
            } else {
                (if end < 0 { n + end } else { end }).clamp(-1, n - 1)
            };
            if lo <= hi {
                return Self::new(0, 0, 1, false);
            }
            (lo, lo + (hi + 1 - lo) / step * step)
        };
        let new_step =
            (self.step as i128 * step as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        let last_value = self.element_at(last);
        Self::new(
            self.element_at(first),
            last_value.saturating_add(new_step),
            new_step,
            false,
        )
    }

    pub fn format(self, duration_parts: bool) -> String {
        let part = |value| {
            if duration_parts {
                duration::format(value)
            } else {
                value.to_string()
            }
        };
        let mut output = format!(
            "{}{}{}",
            part(self.start),
            if self.inclusive { "..=" } else { ".." },
            part(self.end),
        );
        let inferred = if self.start < self.end { 1 } else { -1 };
        if self.step != inferred {
            output.push(':');
            output.push_str(&part(self.step));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::Range;

    #[test]
    fn counts_indexes_slices_and_formats_without_native_handles() {
        let range = Range::new(6, 0, -2, true);
        assert_eq!(range.len(), Some(4));
        assert_eq!(
            [0, 1, 2, 3].map(|index| range.element_at(index)),
            [6, 4, 2, 0]
        );
        assert_eq!(range.slice(1, 3, 1), Range::new(4, 0, -2, false));
        assert_eq!(Range::new(1, 10, 1, false).format(false), "1..10");
        assert_eq!(
            Range::new(0, 1_500_000_000, 500_000_000, false).format(true),
            "0s..1.5s:500ms"
        );
    }
}
