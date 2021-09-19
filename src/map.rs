use duplicate::duplicate;
use std::hint::unreachable_unchecked;
use std::mem::swap;

#[derive(Debug)]
enum Node<V> {
    Leaf {
        key: u64,
        value: V,
    },
    Internal {
        key_prefix: u64,
        branch_bit: u8,
        left: Box<Node<V>>,
        right: Box<Node<V>>,
    },

    // Only used temporarily during insertion
    _TemporaryUnused,
}

#[derive(Debug)]
pub struct PatriciaTreeMap<V> {
    size: usize,
    root: Option<Box<Node<V>>>,
}

impl<V> PatriciaTreeMap<V> {
    pub fn new() -> Self {
        Self {
            size: 0,
            root: None,
        }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get_prefix(key: u64, branch_bit: u8) -> u64 {
        let mask = (1 << branch_bit) - 1;
        key & mask
    }

    fn is_left(key: u64, branch_bit: u8) -> bool {
        key & (1 << branch_bit) == 0
    }

    #[duplicate(
      method                     reference(type);
      [find_insertion_point]     [& type];
      [find_insertion_point_mut] [&mut type];
    )]
    #[allow(clippy::needless_arbitrary_self_type)]
    #[allow(clippy::borrowed_box)]
    fn method(self: reference([Self]), key: u64) -> Option<reference([Box<Node<V>>])> {
        fn aux<V>(node: reference([Box<Node<V>>]), key: u64) -> reference([Box<Node<V>>]) {
            if let Node::Leaf { .. } = **node {
                return node;
            }

            match reference([**node]) {
                Node::Leaf { .. } => unsafe { unreachable_unchecked() },
                Node::Internal {
                    key_prefix,
                    branch_bit,
                    ..
                } => {
                    if *key_prefix != PatriciaTreeMap::<V>::get_prefix(key, *branch_bit) {
                        return node;
                    }
                }
                Node::_TemporaryUnused => unsafe { unreachable_unchecked() },
            }

            match reference([**node]) {
                Node::Leaf { .. } => unsafe { unreachable_unchecked() },
                Node::Internal {
                    branch_bit,
                    left,
                    right,
                    ..
                } => {
                    if PatriciaTreeMap::<V>::is_left(key, *branch_bit) {
                        aux(left, key)
                    } else {
                        aux(right, key)
                    }
                }
                Node::_TemporaryUnused => unsafe { unreachable_unchecked() },
            }
        }

        match reference([self.root]) {
            None => None,
            Some(root) => Some(aux(root, key)),
        }
    }

    pub fn get(&self, key: u64) -> Option<&V> {
        match self.find_insertion_point(key) {
            None => None,
            Some(x) => match x.as_ref() {
                Node::Leaf { key: k, value: v } => {
                    if k == &key {
                        Some(v)
                    } else {
                        None
                    }
                }
                _ => None,
            },
        }
    }

    pub fn contains(&self, key: u64) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(&mut self, key: u64, value: V) -> Option<V> {
        fn aux<V>(tree: &mut PatriciaTreeMap<V>, key: u64, mut value: V) -> Option<V> {
            if tree.root.is_none() {
                tree.root = Some(Box::new(Node::Leaf { key, value }));
                return None;
            }

            fn do_insert<V>(diff: u64, key: u64, value: V, node: &mut Box<Node<V>>) -> Option<V> {
                let branch_bit = diff.trailing_zeros() as u8;
                let key_prefix = PatriciaTreeMap::<V>::get_prefix(key, branch_bit);

                let mut left = Box::new(Node::Leaf { key, value });
                let mut right = Box::new(Node::_TemporaryUnused);

                swap(&mut right, node);

                if !PatriciaTreeMap::<V>::is_left(key, branch_bit) {
                    swap(&mut left, &mut right);
                }

                *node = Box::new(Node::Internal {
                    branch_bit,
                    key_prefix,
                    left,
                    right,
                });

                None
            }

            let node = tree.find_insertion_point_mut(key).unwrap();

            match node.as_mut() {
                Node::Leaf { key: k, .. } => {
                    if k != &key {
                        let diff = *k ^ key;
                        return do_insert(diff, key, value, node);
                    }
                }
                Node::Internal { key_prefix, .. } => {
                    let diff = *key_prefix ^ key;
                    return do_insert(diff, key, value, node);
                }
                Node::_TemporaryUnused => unsafe { unreachable_unchecked() },
            };

            match node.as_mut() {
                Node::Leaf { value: v, .. } => {
                    swap(v, &mut value);
                    Some(value)
                }
                Node::Internal { .. } => unsafe { unreachable_unchecked() },
                Node::_TemporaryUnused => unsafe { unreachable_unchecked() },
            }
        }

        let res = aux(self, key, value);
        self.size += res.is_none() as usize;
        res
    }
}

impl<V> Default for PatriciaTreeMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::PatriciaTreeMap;
    use proptest::bits;
    use proptest::collection::hash_set;
    use proptest::collection::vec;
    use proptest::collection::SizeRange;
    use proptest::prelude::*;
    use std::collections::HashSet;
    use std::hash::Hash;

    #[test]
    fn test_empty_map() {
        let map = PatriciaTreeMap::<String>::new();
        assert_eq!(map.len(), 0);
        assert_eq!(map.get(0), None);
    }

    #[test]
    fn test_insert_return_value() {
        let mut map = PatriciaTreeMap::<String>::new();
        assert_eq!(map.get(123), None);
        assert_eq!(map.insert(123, "A".into()), None);
        assert_eq!(map.get(123), Some(&"A".into()));
        assert_eq!(map.insert(123, "B".into()), Some("A".into()));
        assert_eq!(map.get(123), Some(&"B".into()));
    }

    fn unique_vec<T>(element: T, size: impl Into<SizeRange>) -> impl Strategy<Value = Vec<T::Value>>
    where
        T: Strategy,
        T::Value: Hash + Eq,
    {
        let x = hash_set(element, size);
        x.prop_map(|v| v.into_iter().collect())
    }

    fn test_insertion_impl(keys: Vec<u64>) {
        let tree = {
            let mut tree = PatriciaTreeMap::<String>::new();
            for v in keys.iter() {
                tree.insert(*v, format!("{}", *v));
            }
            tree
        };

        let unique_keys = keys.into_iter().collect::<HashSet<u64>>();

        assert_eq!(tree.len(), unique_keys.len());

        for v in unique_keys.iter() {
            assert_eq!(tree.get(*v), Some(&format!("{}", *v)));
        }
    }

    proptest! {
        #[test]
        fn test_insert_with_duplicates(keys in vec(bits::u64::between(0, 10), 0..100)) {
            test_insertion_impl(keys)
        }

        #[test]
        fn test_insert_unique(keys in unique_vec(bits::u64::between(0, 10), 0..100)) {
            test_insertion_impl(keys)
        }
    }
}