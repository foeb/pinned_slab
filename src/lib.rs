//! This is a slab-allocator inspired by the [`slab`] crate, with the main
//! difference being it allocates pinned fixed sized arrays instead of using a
//! resizable `Vec`. This lets us guarantee that none of the pooled objects will
//! be moved unless we first remove it from the pool.
//!
//! [`slab`]: https://github.com/carllerche/slab

use arrayvec::ArrayVec;
use std::mem;
use std::pin::Pin;

/// The number of elements in each `Chunk`'s array. This can be removed once const
/// generics are stable.
pub const CHUNK_SIZE: usize = 1024;

/// The slab-allocator (also known as an object pool) struct.
#[derive(Debug, Clone)]
pub struct Slab<T> {
    slabs: Vec<Chunk<T>>,
    len: usize,
    next: usize,
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Slab::new()
    }
}

#[derive(Debug, Clone)]
struct Chunk<T> {
    pub entries: Pin<Box<ArrayVec<[Entry<T>; CHUNK_SIZE]>>>,
    pub len: usize,
}

impl<T> Chunk<T> {
    pub fn new() -> Self {
        Chunk {
            entries: Box::pin(ArrayVec::new()),
            len: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum Entry<T> {
    Occupied(T),
    Vacant(usize),
}

impl<T> Slab<T> {
    /// Create a new `Pool`. This doesn't allocate any slabs until the first
    /// value is inserted.
    pub fn new() -> Self {
        Slab {
            slabs: Vec::new(),
            len: 0,
            next: 0,
        }
    }

    /// Get the number of entries currently stored in the `Pool`.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the `Pool` doesn't have any entries.
    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Checks if the `Pool` has an entry for `key`.
    #[allow(unused)]
    pub fn contains(&self, key: usize) -> bool {
        self.get(key).is_some()
    }

    /// Returns the number of elements the `Pool` can contain without allocating
    /// any more slabs.
    #[allow(unused)]
    pub fn capacity(&self) -> usize {
        self.slabs.len() * CHUNK_SIZE
    }

    /// Get a reference to the entry at `key`.
    pub fn get(&self, key: usize) -> Option<&T> {
        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        let slab = self.slabs.get(slab_key)?;
        match slab.entries.get(entry_key) {
            Some(&Entry::Occupied(ref val)) => Some(val),
            _ => None,
        }
    }

    /// Get a mutable reference to the entry at `key`.
    ///
    /// # Safety
    ///
    /// This effectively un-pins the entry at `key`. The caller has to make sure
    /// that this is definitely what they want to do, e.g. they won't invalidate
    /// any pointers to this value.
    pub unsafe fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        let slab = self.slabs.get_mut(slab_key)?;
        let entries = slab.entries.as_mut().get_unchecked_mut();
        match entries.get_mut(entry_key) {
            Some(&mut Entry::Occupied(ref mut val)) => Some(val),
            _ => None,
        }
    }

    /// Insert a value into the `Pool` and get the key for the value.
    pub fn insert(&mut self, val: T) -> usize {
        let key = self.next;

        self.insert_at(key, val);

        key
    }

    fn insert_at(&mut self, key: usize, val: T) {
        self.len += 1;

        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        if slab_key == self.slabs.len() {
            debug_assert_eq!(entry_key, 0);
            self.slabs.push(Chunk::new());
        }

        let slab = self.slabs.get_mut(slab_key).expect("bad pool key: no slab");
        slab.len += 1;

        // SAFETY: This will either push a new `Entry` on to the array, or
        // replace a `Vacant` entry. In either case, this won't move other
        // entries.
        let entries = unsafe { slab.entries.as_mut().get_unchecked_mut() };
        if entry_key == entries.len() {
            entries.push(Entry::Occupied(val));
            self.next = key + 1;
        } else {
            let prev = mem::replace(&mut entries[entry_key], Entry::Occupied(val));
            match prev {
                Entry::Vacant(next) => {
                    self.next = next;
                }
                _ => unreachable!(),
            }
        }
    }

    /// Remove the entry for `key` from the pool.
    ///
    /// # Panics
    ///
    /// This panics if `self.get(key)` is `None`.
    pub fn remove(&mut self, key: usize) -> T {
        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        let slab = self.slabs.get_mut(slab_key).expect("bad pool key: no slab");

        // SAFETY: By calling `remove` on this key, we're giving "permission" to
        // un-pin the entry. Since `mem::replace` will only affect that entry,
        // all other entries remain pinned.
        let entries = unsafe { slab.entries.as_mut().get_unchecked_mut() };
        let prev = mem::replace(&mut entries[entry_key], Entry::Vacant(self.next));
        match prev {
            Entry::Occupied(val) => {
                slab.len -= 1;
                self.len -= 1;
                self.next = key;
                val
            }
            _ => {
                entries[key] = prev;
                panic!("invalid key");
            }
        }
    }

    /// Free any empty slabs.
    pub fn free_unused(&mut self) {
        self.slabs.retain(|slab| slab.len > 0)
    }
}
