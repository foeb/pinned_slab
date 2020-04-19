//! This is a slab-allocator inspired by the [`slab`] crate, with the main
//! difference being it allocates pinned fixed sized arrays instead of using a
//! resizable `Vec`. This lets us guarantee that none of the pooled objects will
//! be moved unless we first remove it from the pool.
//!
//! [`slab`]: https://github.com/carllerche/slab

use arrayvec::ArrayVec;
use std::iter::{IntoIterator, Iterator};
use std::mem;
use std::ops;
use std::pin::Pin;

/// The number of elements in each `Chunk`'s array. This can be removed once const
/// generics are stable.
pub const CHUNK_SIZE: usize = 1024;

/// The slab-allocator (also known as an object pool) struct.
#[derive(Debug, Clone)]
pub struct Slab<T> {
    chunks: Vec<Chunk<T>>,
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

/// An iterator over the values stored in the `Slab`
pub struct Iter<'a, T: 'a> {
    chunks: std::slice::Iter<'a, Chunk<T>>,
    entries: std::slice::Iter<'a, Entry<T>>,
    curr: usize,
}

/// An iterator over the values stored in the `Slab`
pub struct IterMut<'a, T: 'a> {
    chunks: std::slice::IterMut<'a, Chunk<T>>,
    entries: std::slice::IterMut<'a, Entry<T>>,
    curr: usize,
}

impl<T> Slab<T> {
    /// Construct a new, empty `Slab`.
    ///
    /// The function does not allocate and the returned slab will have no
    /// capacity until `insert` is called or capacity is explicitly reserved.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let slab: Slab<i32> = Slab::new();
    /// ```
    pub fn new() -> Self {
        Slab {
            chunks: Vec::new(),
            len: 0,
            next: 0,
        }
    }

    /// Return the number of stored values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    ///
    /// for i in 0..3 {
    ///     slab.insert(i);
    /// }
    ///
    /// assert_eq!(3, slab.len());
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return `true` if there are no values stored in the slab.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    /// assert!(slab.is_empty());
    ///
    /// slab.insert(1);
    /// assert!(!slab.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return `true` if a value is associated with the given key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    ///
    /// let (hello, _) = slab.insert("hello");
    /// assert!(slab.contains(hello));
    ///
    /// slab.remove(hello);
    ///
    /// assert!(!slab.contains(hello));
    /// ```
    pub fn contains(&self, key: usize) -> bool {
        self.get(key).is_some()
    }

    /// Return the number of values the slab can store without reallocating.
    ///
    /// This will always be a multiple of `CHUNK_SIZE`.
    pub fn capacity(&self) -> usize {
        self.chunks.len() * CHUNK_SIZE
    }

    /// Return an iterator over the slab.
    ///
    /// This function should generally be **avoided** as it is not efficient.
    /// Iterators must iterate over every slot in the slab even if it is
    /// vacant. As such, a slab with a capacity of 1 million but only one
    /// stored value must still iterate the million slots.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    ///
    /// for i in 0..3 {
    ///     slab.insert(i);
    /// }
    ///
    /// let mut iterator = slab.iter();
    ///
    /// assert_eq!(iterator.next(), Some((0, &0)));
    /// assert_eq!(iterator.next(), Some((1, &1)));
    /// assert_eq!(iterator.next(), Some((2, &2)));
    /// assert_eq!(iterator.next(), None);
    /// ```
    pub fn iter(&self) -> Iter<T> {
        Iter {
            chunks: self.chunks.iter(),
            entries: [].iter(),
            curr: 0,
        }
    }

    /// Return an iterator that allows modifying each value.
    ///
    /// This function should generally be **avoided** as it is not efficient.
    /// Iterators must iterate over every slot in the slab even if it is
    /// vacant. As such, a slab with a capacity of 1 million but only one
    /// stored value must still iterate the million slots.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    ///
    /// let (key1, _) = slab.insert(0);
    /// let (key2, _) = slab.insert(1);
    ///
    /// for (key, val) in unsafe { slab.iter_mut() } {
    ///     if key == key1 {
    ///         *val += 2;
    ///     }
    /// }
    ///
    /// assert_eq!(slab[key1], 2);
    /// assert_eq!(slab[key2], 1);
    /// ```
    ///
    /// # Safety
    ///
    /// This effectively un-pins every entry. The caller has to make sure
    /// that this is definitely what they want to do, e.g. they won't invalidate
    /// any pointers to these values.
    pub unsafe fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            chunks: self.chunks.iter_mut(),
            entries: [].iter_mut(),
            curr: 0,
        }
    }

    /// Return a reference to the value associated with the given key.
    ///
    /// If the given key is not associated with a value, then `None` is
    /// returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    /// let (key, _) = slab.insert("hello");
    ///
    /// assert_eq!(slab.get(key), Some(&"hello"));
    /// assert_eq!(slab.get(123), None);
    /// ```
    pub fn get(&self, key: usize) -> Option<&T> {
        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        let slab = self.chunks.get(slab_key)?;
        match slab.entries.get(entry_key) {
            Some(&Entry::Occupied(ref val)) => Some(val),
            _ => None,
        }
    }

    /// Return a mutable reference to the value associated with the given key.
    ///
    /// If the given key is not associated with a value, then `None` is
    /// returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    /// let (key, _) = slab.insert("hello");
    ///
    /// unsafe {
    ///     *slab.get_mut(key).unwrap() = "world";
    /// }
    ///
    /// assert_eq!(*unsafe { slab.get_mut(key) }.unwrap(), "world");
    /// assert_eq!(unsafe { slab.get_mut(123) }, None);
    /// ```
    ///
    /// # Safety
    ///
    /// This effectively un-pins the entry at `key`. The caller has to make sure
    /// that this is definitely what they want to do, e.g. they won't invalidate
    /// any pointers to this value.
    pub unsafe fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        let slab = self.chunks.get_mut(slab_key)?;
        let entries = slab.entries.as_mut().get_unchecked_mut();
        match entries.get_mut(entry_key) {
            Some(&mut Entry::Occupied(ref mut val)) => Some(val),
            _ => None,
        }
    }

    /// Insert a value in the slab, returning key assigned to the value and a
    /// reference to that value.
    ///
    /// The returned key can later be used to retrieve or remove the value using indexed
    /// lookup and `remove`. Additional capacity is allocated if needed. See
    /// [Capacity and reallocation](index.html#capacity-and-reallocation).
    ///
    /// # Panics
    ///
    /// Panics if the number of elements in the vector overflows a `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    /// let (key, value) = slab.insert("hello");
    /// assert_eq!(*value, "hello");
    /// assert_eq!(slab[key], "hello");
    /// ```
    /// Insert a value into the `Slab` and get the key for the value.
    pub fn insert(&mut self, val: T) -> (usize, &T) {
        let key = self.next;

        (key, self.insert_at(key, val))
    }

    fn insert_at(&mut self, key: usize, val: T) -> &T {
        self.len += 1;

        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        if slab_key == self.chunks.len() {
            debug_assert_eq!(entry_key, 0);
            self.chunks.push(Chunk::new());
        }

        let slab = self.chunks.get_mut(slab_key).expect("invalid key");
        slab.len += 1;

        // SAFETY: This will either push a new `Entry` on to the array, or
        // replace a `Vacant` entry. In either case, this won't move other
        // entries.
        let entries = unsafe { slab.entries.as_mut().get_unchecked_mut() };
        if entry_key == entries.len() {
            entries.push(Entry::Occupied(val));
            self.next = key + 1;
            match entries.get(entries.len() - 1) {
                Some(Entry::Occupied(ref v)) => v,
                _ => unreachable!(),
            }
        } else {
            let entry = &mut entries[entry_key];
            let prev = mem::replace(entry, Entry::Occupied(val));
            match prev {
                Entry::Vacant(next) => {
                    self.next = next;
                }
                _ => unreachable!(),
            }
            match entry {
                Entry::Occupied(ref v) => v,
                _ => unreachable!(),
            }
        }
    }

    /// Remove and return the value associated with the given key.
    ///
    /// The key is then released and may be associated with future stored
    /// values.
    ///
    /// # Panics
    ///
    /// Panics if `key` is not associated with a value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    ///
    /// let (hello, _) = slab.insert("hello");
    ///
    /// assert_eq!(slab.remove(hello), "hello");
    /// assert!(!slab.contains(hello));
    /// ```
    pub fn remove(&mut self, key: usize) -> T {
        let slab_key = key / CHUNK_SIZE;
        let entry_key = key % CHUNK_SIZE;

        let chunk = self.chunks.get_mut(slab_key).expect("invalid key");

        // SAFETY: By calling `remove` on this key, we're giving "permission" to
        // un-pin the entry. Since `mem::replace` will only affect that entry,
        // all other entries remain pinned.
        let entries = unsafe { chunk.entries.as_mut().get_unchecked_mut() };
        let prev = mem::replace(&mut entries[entry_key], Entry::Vacant(self.next));
        match prev {
            Entry::Occupied(val) => {
                chunk.len -= 1;
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

    /// Free any empty chunks.
    pub fn free_unused(&mut self) {
        self.chunks.retain(|slab| slab.len > 0)
    }

    /// Retain only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(usize, &mut e)`
    /// returns false. This method operates in place and preserves the key
    /// associated with the retained values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pinned_slab::*;
    /// let mut slab = Slab::new();
    ///
    /// let (k1, _) = slab.insert(0);
    /// let (k2, _) = slab.insert(1);
    /// let (k3, _) = slab.insert(2);
    ///
    /// unsafe {
    ///    slab.retain(|key, val| key == k1 || *val == 1);
    /// }
    ///
    /// assert!(slab.contains(k1));
    /// assert!(slab.contains(k2));
    /// assert!(!slab.contains(k3));
    ///
    /// assert_eq!(2, slab.len());
    /// ```
    ///
    /// # Safety
    ///
    /// This effectively un-pins every entry. The caller has to make sure
    /// that this is definitely what they want to do, e.g. they won't invalidate
    /// any pointers to these values.
    pub unsafe fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(usize, &mut T) -> bool,
    {
        for i in 0..self.chunks.len() {
            for j in 0..CHUNK_SIZE {
                let chunk = self.chunks.get_mut(i).unwrap();
                let entry = chunk.entries.as_mut().get_unchecked_mut().get_mut(j);
                let key = i * CHUNK_SIZE + j;
                let keep = match entry {
                    Some(Entry::Occupied(ref mut v)) => f(key, v),
                    None => break,
                    _ => true,
                };

                if !keep {
                    self.remove(key);
                }
            }
        }
    }
}

impl<T> ops::Index<usize> for Slab<T> {
    type Output = T;

    fn index(&self, key: usize) -> &T {
        match self.get(key) {
            Some(&ref v) => v,
            _ => panic!("invalid key"),
        }
    }
}

impl<'a, T> IntoIterator for &'a Slab<T> {
    type Item = (usize, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (usize, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while let Some(entry) = self.entries.next() {
                let curr = self.curr;
                self.curr += 1;

                if let Entry::Occupied(ref v) = *entry {
                    return Some((curr, v));
                }
            }

            // `self.entries.next()` was `None`...
            if let Some(chunk) = self.chunks.next() {
                self.entries = chunk.entries.iter();
            } else {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.chunks.len() * CHUNK_SIZE))
    }
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (usize, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while let Some(entry) = self.entries.next() {
                let curr = self.curr;
                self.curr += 1;

                if let Entry::Occupied(ref mut v) = *entry {
                    return Some((curr, v));
                }
            }

            // `self.entries.next()` was `None`...
            if let Some(chunk) = self.chunks.next() {
                let entries = chunk.entries.as_mut();
                self.entries = unsafe { entries.get_unchecked_mut().iter_mut() };
            } else {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.chunks.len() * CHUNK_SIZE))
    }
}
