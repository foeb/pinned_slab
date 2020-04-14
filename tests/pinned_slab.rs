use pinned_slab::*;

#[test]
fn insert_get_remove_one() {
    let mut pool = Slab::new();
    assert!(pool.is_empty());

    let key = pool.insert(10);

    assert_eq!(pool.get(key), Some(&10));
    assert!(!pool.is_empty());
    assert!(pool.contains(key));

    assert_eq!(pool.remove(key), 10);
    assert!(!pool.contains(key));
    assert!(pool.get(key).is_none());
}

#[test]
fn insert_get_many() {
    let mut pool = Slab::new();

    for i in 0..CHUNK_SIZE {
        let key = pool.insert(i + 10);
        assert_eq!(*pool.get(key).unwrap(), i + 10);
    }

    assert_eq!(pool.capacity(), CHUNK_SIZE);

    // Storing another one grows the pool
    let key = pool.insert(20);
    assert_eq!(*pool.get(key).unwrap(), 20);

    // Capacity grows by 2x
    assert_eq!(pool.capacity(), 2 * CHUNK_SIZE);
}

#[test]
fn insert_get_remove_many() {
    let mut pool = Slab::new();
    let mut keys = vec![];

    for i in 0..CHUNK_SIZE {
        for j in 0..10 {
            let val = (i * 10) + j;

            let key = pool.insert(val);
            keys.push((key, val));
            assert_eq!(*pool.get(key).unwrap(), val);
        }

        for (key, val) in keys.drain(..) {
            assert_eq!(val, pool.remove(key));
        }
    }

    assert_eq!(CHUNK_SIZE, pool.capacity());
}

#[test]
#[should_panic]
fn double_remove_panics() {
    let mut pool = Slab::<usize>::new();
    let key = pool.insert(123);
    pool.remove(key);
    pool.remove(key);
}

#[test]
#[should_panic]
fn invalid_remove_panics() {
    let mut pool = Slab::<usize>::new();
    pool.remove(0);
}

#[test]
fn free_unused() {
    let mut pool = Slab::new();
    let key = pool.insert(123);

    assert_eq!(CHUNK_SIZE, pool.capacity());

    pool.remove(key);
    pool.free_unused();

    assert_eq!(0, pool.capacity());
}
