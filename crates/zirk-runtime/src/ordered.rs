//! Hash lookup with insertion-ordered traversal and constant-time unlinking.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_INSERT_RESERVATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn fail_next_insert_reservation_for_tests() {
    FAIL_NEXT_INSERT_RESERVATION.with(|fail| fail.set(true));
}

pub(crate) trait TryKeyClone: Eq + Hash + Sized {
    fn try_clone_key(&self) -> Result<Self, ()>;
}

impl TryKeyClone for Vec<u8> {
    fn try_clone_key(&self) -> Result<Self, ()> {
        let mut copy = Vec::new();
        copy.try_reserve_exact(self.len()).map_err(|_| ())?;
        copy.extend_from_slice(self);
        Ok(copy)
    }
}

impl TryKeyClone for Option<Box<str>> {
    fn try_clone_key(&self) -> Result<Self, ()> {
        self.as_deref()
            .map(|text| {
                let mut copy = String::new();
                copy.try_reserve_exact(text.len()).map_err(|_| ())?;
                copy.push_str(text);
                Ok(copy.into_boxed_str())
            })
            .transpose()
    }
}

/// A typed aggregate key whose text fields use NFC content equality. `raw`
/// retains the key's physical layout with private immutable text handles for
/// iteration; `canonical` zeros those pointer slots; `text` participates in
/// both equality and hashing by content.
pub(crate) struct CompositeKey {
    pub(crate) raw: Vec<u8>,
    canonical: Vec<u8>,
    text: Vec<Option<Box<str>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct CollectionTextField {
    pub(crate) offset: usize,
    pub(crate) presence_guards: Box<[usize]>,
    pub(crate) tag_guards: Box<[(usize, u32)]>,
}

pub(crate) struct TemporaryRoots(Vec<u64>);

impl Drop for TemporaryRoots {
    fn drop(&mut self) {
        for root in self.0.iter().copied() {
            crate::collector::zirk_rt_global_root_remove(root);
        }
    }
}

impl TemporaryRoots {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn push(&mut self, handle: *mut std::ffi::c_void) -> Result<(), ()> {
        // Reserve first: once the collector returns a root id, every error
        // path must still retain the id so Drop can release it.
        self.0.try_reserve(1).map_err(|_| ())?;
        let root = unsafe { crate::collector::zirk_rt_global_root_add(handle) };
        if root == 0 {
            return Err(());
        }
        self.0.push(root);
        Ok(())
    }
}

pub(crate) unsafe fn collection_text_fields(
    pointer: *const usize,
    word_count: usize,
    value_size: usize,
) -> Box<[CollectionTextField]> {
    if word_count == 0 {
        return Box::new([]);
    }
    assert!(!pointer.is_null(), "collection text-field metadata is null");
    let words = unsafe { std::slice::from_raw_parts(pointer, word_count) };
    let mut fields = Vec::new();
    let mut cursor = 0;
    while cursor < words.len() {
        assert!(
            words.len() - cursor >= 2,
            "truncated collection text-field metadata"
        );
        let offset = words[cursor];
        let guard_count = words[cursor + 1];
        cursor += 2;
        assert!(
            offset
                .checked_add(std::mem::size_of::<usize>())
                .is_some_and(|end| end <= value_size),
            "collection text field is outside its value layout"
        );
        assert!(
            guard_count <= 64,
            "collection text field has too many nullable guards"
        );
        assert!(
            guard_count <= words.len() - cursor,
            "truncated collection text-field presence guards"
        );
        let presence_guards = words[cursor..cursor + guard_count]
            .to_vec()
            .into_boxed_slice();
        cursor += guard_count;
        for guard in &presence_guards {
            assert!(
                *guard < value_size,
                "collection nullable guard is outside its value layout"
            );
        }
        assert!(
            cursor < words.len(),
            "missing collection text-field tag-guard count"
        );
        let tag_guard_count = words[cursor];
        cursor += 1;
        assert!(
            tag_guard_count
                .checked_mul(2)
                .is_some_and(|count| count <= words.len() - cursor),
            "truncated collection text-field tag guards"
        );
        let mut tag_guards = Vec::with_capacity(tag_guard_count);
        for _ in 0..tag_guard_count {
            let tag_offset = words[cursor];
            let expected =
                u32::try_from(words[cursor + 1]).expect("collection text-field tag exceeds u32");
            cursor += 2;
            assert!(
                tag_offset
                    .checked_add(4)
                    .is_some_and(|end| end <= value_size),
                "collection text-field tag guard is outside its value layout"
            );
            tag_guards.push((tag_offset, expected));
        }
        fields.push(CollectionTextField {
            offset,
            presence_guards,
            tag_guards: tag_guards.into_boxed_slice(),
        });
    }
    fields.into_boxed_slice()
}

impl CompositeKey {
    pub(crate) fn from_bytes(value: &[u8], fields: &[CollectionTextField]) -> Result<Self, ()> {
        let mut raw = Vec::new();
        raw.try_reserve_exact(value.len()).map_err(|_| ())?;
        raw.extend_from_slice(value);
        let mut canonical = Vec::new();
        canonical.try_reserve_exact(value.len()).map_err(|_| ())?;
        canonical.extend_from_slice(value);
        let mut text = Vec::new();
        text.try_reserve_exact(fields.len()).map_err(|_| ())?;
        for field in fields {
            let active = field
                .presence_guards
                .iter()
                .all(|offset| value[*offset] != 0)
                && field.tag_guards.iter().all(|(offset, expected)| {
                    let mut tag = [0_u8; 4];
                    tag.copy_from_slice(&value[*offset..*offset + 4]);
                    u32::from_ne_bytes(tag) == *expected
                });
            let pointer = value
                .get(field.offset..field.offset + std::mem::size_of::<usize>())
                .ok_or(())?;
            let handle =
                usize::from_ne_bytes(pointer.try_into().map_err(|_| ())?) as *mut std::ffi::c_void;
            let snapshot = if active && !handle.is_null() {
                let contents = unsafe { (*crate::string::payload_ptr(handle)).as_str() };
                Some(zirk_runtime_core::text::normalize(contents, "NFC").into_boxed_str())
            } else {
                None
            };
            text.push(snapshot);
            canonical[field.offset..field.offset + std::mem::size_of::<usize>()].fill(0);
            if !active {
                raw[field.offset..field.offset + std::mem::size_of::<usize>()].fill(0);
            }
        }
        Ok(Self {
            raw,
            canonical,
            text,
        })
    }

    /// Stores private, independently mutable String handles in the physical
    /// key bytes. Root them until the collection owns those bytes.
    pub(crate) fn snapshot_handles(
        &mut self,
        fields: &[CollectionTextField],
    ) -> Result<TemporaryRoots, ()> {
        let mut roots = TemporaryRoots::new();
        for (field, snapshot) in fields.iter().zip(&self.text) {
            let handle = snapshot
                .as_deref()
                .map(crate::string::alloc_owned)
                .unwrap_or(std::ptr::null_mut());
            if !handle.is_null() {
                roots.push(handle)?;
            }
            self.raw[field.offset..field.offset + std::mem::size_of::<usize>()]
                .copy_from_slice(&(handle as usize).to_ne_bytes());
        }
        Ok(roots)
    }

    /// Materializes a fresh key snapshot for an Array result so user mutation
    /// of a returned String cannot change the key retained by the collection.
    pub(crate) fn materialize(
        &self,
        fields: &[CollectionTextField],
    ) -> Result<(Vec<u8>, TemporaryRoots), ()> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.canonical.len())
            .map_err(|_| ())?;
        bytes.extend_from_slice(&self.canonical);
        let mut roots = TemporaryRoots::new();
        for (field, snapshot) in fields.iter().zip(&self.text) {
            let handle = snapshot
                .as_deref()
                .map(crate::string::alloc_owned)
                .unwrap_or(std::ptr::null_mut());
            if !handle.is_null() {
                roots.push(handle)?;
            }
            bytes[field.offset..field.offset + std::mem::size_of::<usize>()]
                .copy_from_slice(&(handle as usize).to_ne_bytes());
        }
        Ok((bytes, roots))
    }
}

impl PartialEq for CompositeKey {
    fn eq(&self, other: &Self) -> bool {
        self.canonical == other.canonical && self.text == other.text
    }
}

impl Eq for CompositeKey {}

impl Hash for CompositeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canonical.hash(state);
        self.text.hash(state);
    }
}

impl TryKeyClone for CompositeKey {
    fn try_clone_key(&self) -> Result<Self, ()> {
        let mut raw = Vec::new();
        raw.try_reserve_exact(self.raw.len()).map_err(|_| ())?;
        raw.extend_from_slice(&self.raw);
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(self.canonical.len())
            .map_err(|_| ())?;
        canonical.extend_from_slice(&self.canonical);
        let mut text = Vec::new();
        text.try_reserve_exact(self.text.len()).map_err(|_| ())?;
        for value in &self.text {
            let copy = value
                .as_deref()
                .map(|value| {
                    let mut copy = String::new();
                    copy.try_reserve_exact(value.len()).map_err(|_| ())?;
                    copy.push_str(value);
                    Ok::<_, ()>(copy.into_boxed_str())
                })
                .transpose()?;
            text.push(copy);
        }
        Ok(Self {
            raw,
            canonical,
            text,
        })
    }
}

struct Node<K, V> {
    key: K,
    value: V,
    previous: Option<usize>,
    next: Option<usize>,
}

pub(crate) struct OrderedMap<K, V> {
    index: HashMap<K, usize>,
    nodes: Vec<Option<Node<K, V>>>,
    free: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            index: HashMap::new(),
            nodes: Vec::new(),
            free: Vec::new(),
            head: None,
            tail: None,
        }
    }
}

impl<K: TryKeyClone, V> OrderedMap<K, V> {
    pub(crate) fn len(&self) -> usize {
        self.index.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.index
            .get(key)
            .and_then(|&index| self.nodes[index].as_ref())
            .map(|node| &node.value)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Result<Option<V>, ()> {
        if let Some(&index) = self.index.get(&key) {
            let node = self.nodes[index].as_mut().expect("indexed node exists");
            return Ok(Some(std::mem::replace(&mut node.value, value)));
        }

        #[cfg(test)]
        if FAIL_NEXT_INSERT_RESERVATION.with(std::cell::Cell::get) {
            FAIL_NEXT_INSERT_RESERVATION.with(|fail| fail.set(false));
            return Err(());
        }

        self.index.try_reserve(1).map_err(|_| ())?;
        self.free.try_reserve(1).map_err(|_| ())?;
        if self.free.is_empty() {
            self.nodes.try_reserve(1).map_err(|_| ())?;
        }

        let node_key = key.try_clone_key()?;
        let index = self.free.pop().unwrap_or(self.nodes.len());
        let previous = self.tail;
        let node = Node {
            key: node_key,
            value,
            previous,
            next: None,
        };
        if index == self.nodes.len() {
            self.nodes.push(Some(node));
        } else {
            self.nodes[index] = Some(node);
        }
        if let Some(previous) = previous {
            self.nodes[previous]
                .as_mut()
                .expect("tail node exists")
                .next = Some(index);
        } else {
            self.head = Some(index);
        }
        self.tail = Some(index);
        self.index.insert(key, index);
        Ok(None)
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.index.remove(key)?;
        let node = self.nodes[index].take().expect("indexed node exists");
        if let Some(previous) = node.previous {
            self.nodes[previous]
                .as_mut()
                .expect("previous node exists")
                .next = node.next;
        } else {
            self.head = node.next;
        }
        if let Some(next) = node.next {
            self.nodes[next]
                .as_mut()
                .expect("next node exists")
                .previous = node.previous;
        } else {
            self.tail = node.previous;
        }
        self.free.push(index);
        Some(node.value)
    }

    pub(crate) fn clear(&mut self) {
        self.index.clear();
        self.nodes.clear();
        self.free.clear();
        self.head = None;
        self.tail = None;
    }

    pub(crate) fn iter(&self) -> OrderedIter<'_, K, V> {
        OrderedIter {
            map: self,
            next: self.head,
        }
    }
}

pub(crate) struct OrderedIter<'a, K, V> {
    map: &'a OrderedMap<K, V>,
    next: Option<usize>,
}

impl<'a, K, V> Iterator for OrderedIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.next?;
        let node = self.map.nodes[index].as_ref().expect("linked node exists");
        self.next = node.next;
        Some((&node.key, &node.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Eq, Hash, PartialEq)]
    struct CloneGate(u8, bool);

    impl TryKeyClone for CloneGate {
        fn try_clone_key(&self) -> Result<Self, ()> {
            if self.1 {
                Err(())
            } else {
                Ok(Self(self.0, false))
            }
        }
    }

    #[test]
    fn failed_key_copy_leaves_existing_ordered_entries_unchanged() {
        let mut entries = OrderedMap::default();
        entries.insert(CloneGate(1, false), 10).unwrap();

        assert!(entries.insert(CloneGate(2, true), 20).is_err());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries.get(&CloneGate(1, false)), Some(&10));
        assert_eq!(
            entries
                .iter()
                .map(|(key, value)| (key.0, *value))
                .collect::<Vec<_>>(),
            [(1, 10)]
        );
    }

    #[test]
    fn composite_key_equality_implies_equal_hash_across_generated_cases() {
        use std::collections::hash_map::DefaultHasher;

        for seed in 0_u64..512 {
            let canonical = seed
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .to_ne_bytes()
                .repeat((seed as usize % 5) + 1);
            let text = vec![
                Some(format!("snapshot-{}", seed % 19).into_boxed_str()),
                (seed % 3 != 0).then(|| format!("nested-{}", seed % 7).into_boxed_str()),
            ];
            let mut left_raw = canonical.clone();
            let mut right_raw = canonical.clone();
            left_raw.push(seed as u8);
            right_raw.push((seed as u8).wrapping_add(1));

            let left = CompositeKey {
                raw: left_raw,
                canonical: canonical.clone(),
                text: text.clone(),
            };
            let right = CompositeKey {
                raw: right_raw,
                canonical,
                text,
            };
            assert!(left == right, "generated equivalent key at seed {seed}");

            let mut left_hash = DefaultHasher::new();
            left.hash(&mut left_hash);
            let mut right_hash = DefaultHasher::new();
            right.hash(&mut right_hash);
            assert_eq!(
                left_hash.finish(),
                right_hash.finish(),
                "equal generated keys must hash equally at seed {seed}"
            );

            let mut map = OrderedMap::default();
            assert_eq!(map.insert(left, seed).unwrap(), None);
            assert_eq!(map.get(&right), Some(&seed));
            assert_eq!(map.insert(right, seed + 1).unwrap(), Some(seed));
            assert_eq!(map.len(), 1);
        }
    }
}
