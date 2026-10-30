//! Platform-neutral operations for the delivered collection surface.
//!
//! Native collection handles and interpreter heap entries own storage and
//! identity.  This module deliberately owns neither: it centralizes only the
//! value operations both engines must agree on.

use std::collections::{HashMap, HashSet};

/// Recursive semantic capability required for a value to be used as a
/// `Map` key or `Set` element. The compiler builds this from the concrete
/// instantiated type; runtimes use the same shape to select lawful hashing
/// and equality behavior instead of inferring semantics from byte size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionHashEq {
    Integer {
        bits: u16,
        signed: bool,
    },
    BinaryFloat {
        bits: u16,
    },
    Decimal,
    Boolean,
    /// NFC-normalized content, snapshotted when inserted into a collection.
    Text(CollectionTextKind),
    /// Stable managed-object identity. Mutable fields do not participate.
    Identity,
    Nullable(Box<CollectionHashEq>),
    Tuple(Vec<CollectionHashEq>),
    /// One field-capability list per canonical variant discriminant.
    Enum(Vec<Vec<CollectionHashEq>>),
    /// One payload capability per canonical tagged-union alternative.
    Union(Vec<CollectionHashEq>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionTextKind {
    String,
    Char,
}

/// Resolves an element index, with negative indexes measured from the end.
pub fn element_index(length: usize, index: i64) -> Option<usize> {
    let length = i64::try_from(length).ok()?;
    let index = if index < 0 {
        length.checked_add(index)?
    } else {
        index
    };
    (0..length).contains(&index).then_some(index as usize)
}

/// Resolves an insertion index.  Unlike [`element_index`], the end position
/// is valid so an insertion can append.
pub fn insertion_index(length: usize, index: i64) -> Option<usize> {
    let length = i64::try_from(length).ok()?;
    let index = if index < 0 {
        length.checked_add(index)?
    } else {
        index
    };
    (0..=length).contains(&index).then_some(index as usize)
}

/// Returns the selected indexes using Zirk's Python-compatible slice rules.
/// `i64::MIN` denotes an omitted bound or step.
pub fn slice_indices(
    length: usize,
    start: i64,
    end: i64,
    step: i64,
) -> Result<Vec<usize>, &'static str> {
    const MISSING: i64 = i64::MIN;
    let length = i64::try_from(length).map_err(|_| "collection is too large")?;
    let step = if step == MISSING { 1 } else { step };
    if step == 0 {
        return Err("a range's step cannot be zero");
    }
    if length == 0 {
        return Ok(Vec::new());
    }

    let forward = |bound: i64| {
        if bound < 0 {
            (length + bound).max(0)
        } else {
            bound.min(length)
        }
    };
    let mut indexes = Vec::new();
    if step > 0 {
        let mut index = if start == MISSING { 0 } else { forward(start) };
        let end = if end == MISSING { length } else { forward(end) };
        while index < end {
            indexes.push(index as usize);
            index = index.saturating_add(step);
        }
    } else {
        let mut index = if start == MISSING {
            length - 1
        } else {
            (if start < 0 { length + start } else { start }).clamp(-1, length - 1)
        };
        let end = if end == MISSING {
            -1
        } else {
            (if end < 0 { length + end } else { end }).clamp(-1, length - 1)
        };
        while index > end {
            indexes.push(index as usize);
            index = index.saturating_add(step);
        }
    }
    Ok(indexes)
}

/// Shared word-value map used by the currently delivered native map ABI.
pub type WordMap = HashMap<i64, i64>;
/// Shared word-value set used by the currently delivered native set ABI.
pub type WordSet = HashSet<i64>;

pub fn map_set(map: &mut WordMap, key: i64, value: i64) {
    map.insert(key, value);
}
pub fn map_get(map: &WordMap, key: i64) -> Option<i64> {
    map.get(&key).copied()
}
pub fn map_remove(map: &mut WordMap, key: i64) -> bool {
    map.remove(&key).is_some()
}
pub fn set_add(set: &mut WordSet, value: i64) -> bool {
    set.insert(value)
}
pub fn set_remove(set: &mut WordSet, value: i64) -> bool {
    set.remove(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_and_slices_match_delivered_collection_rules() {
        assert_eq!(element_index(3, -1), Some(2));
        assert_eq!(element_index(3, -4), None);
        assert_eq!(insertion_index(3, 3), Some(3));
        assert_eq!(
            slice_indices(5, i64::MIN, i64::MIN, -1).unwrap(),
            [4, 3, 2, 1, 0]
        );
        assert_eq!(slice_indices(5, 1, -1, 2).unwrap(), [1, 3]);
        assert!(slice_indices(5, 0, 5, 0).is_err());
    }

    #[test]
    fn word_maps_and_sets_preserve_update_and_membership_semantics() {
        let mut map = WordMap::new();
        map_set(&mut map, 1, 10);
        map_set(&mut map, 1, 20);
        assert_eq!(map_get(&map, 1), Some(20));
        assert!(map_remove(&mut map, 1));
        assert!(!map_remove(&mut map, 1));

        let mut set = WordSet::new();
        assert!(set_add(&mut set, 7));
        assert!(!set_add(&mut set, 7));
        assert!(set_remove(&mut set, 7));
    }
}
