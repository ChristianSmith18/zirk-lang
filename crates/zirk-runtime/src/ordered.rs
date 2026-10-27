//! Hash lookup with insertion-ordered traversal and constant-time unlinking.

use std::collections::HashMap;
use std::hash::Hash;

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
}
