use pinned_slab::*;

#[test]
fn insert_get_remove_one() {
    let mut slab = Slab::new();
    assert!(slab.is_empty());

    let (key, _) = slab.insert(10);

    assert_eq!(slab[key], 10);
    assert_eq!(slab.get(key), Some(&10));
    assert!(!slab.is_empty());
    assert!(slab.contains(key));

    assert_eq!(slab.remove(key), 10);
    assert!(!slab.contains(key));
    assert!(slab.get(key).is_none());
}

#[test]
fn insert_get_many() {
    let mut slab = Slab::new();

    for i in 0..CHUNK_SIZE {
        let (key, _) = slab.insert(i + 10);
        assert_eq!(slab[key], i + 10);
    }

    assert_eq!(slab.capacity(), CHUNK_SIZE);

    // Storing another one grows the slab
    let (key, _) = slab.insert(20);
    assert_eq!(slab[key], 20);

    // Capacity grows by 2x
    assert_eq!(slab.capacity(), 2 * CHUNK_SIZE);
}

#[test]
fn insert_get_remove_many() {
    let mut slab = Slab::new();
    let mut keys = vec![];

    for i in 0..10 {
        for j in 0..CHUNK_SIZE {
            let val = (i * 10) + j;

            let (key, _) = slab.insert(val);
            keys.push((key, val));
            assert_eq!(slab[key], val);
        }

        for (key, val) in keys.drain(..) {
            assert_eq!(val, slab.remove(key));
        }
    }

    assert_eq!(CHUNK_SIZE, slab.capacity());
}

#[test]
#[should_panic(expected = "invalid key")]
fn invalid_get_panics() {
    let slab = Slab::<usize>::new();
    let _ = &slab[0];
}

#[test]
#[should_panic(expected = "invalid key")]
fn double_remove_panics() {
    let mut slab = Slab::<usize>::new();
    let (key, _) = slab.insert(123);
    slab.remove(key);
    slab.remove(key);
}

#[test]
#[should_panic(expected = "invalid key")]
fn invalid_remove_panics() {
    let mut slab = Slab::<usize>::new();
    slab.remove(0);
}

#[test]
fn slab_get_mut() {
    let mut slab = Slab::new();
    let (key, _) = slab.insert(1);

    unsafe {
        *slab.get_mut(key).unwrap() = 2;
    }
    assert_eq!(slab[key], 2);
}

#[test]
fn retain() {
    let mut slab = Slab::new();

    let (key1, _) = slab.insert(0);
    let (key2, _) = slab.insert(1);

    unsafe {
        slab.retain(|key, x| {
            assert_eq!(key, *x);
            *x % 2 == 0
        })
    };

    assert_eq!(slab.len(), 1);
    assert_eq!(slab[key1], 0);
    assert!(!slab.contains(key2));

    // Ensure consistency is retained
    let (key, _) = slab.insert(123);
    assert_eq!(key, key2);
    assert_eq!(2, slab.len());
}

#[test]
fn iter() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }

    let vals: Vec<_> = slab
        .iter()
        .enumerate()
        .map(|(i, (key, val))| {
            assert_eq!(i, key);
            *val
        })
        .collect();
    assert_eq!(vals, vec![0, 1, 2, 3]);

    slab.remove(1);

    let vals: Vec<_> = slab.iter().map(|(_, r)| *r).collect();
    assert_eq!(vals, vec![0, 2, 3]);
}

#[test]
fn iter_mut() {
    let mut slab = Slab::new();

    for i in 0..4 {
        slab.insert(i);
    }

    for (i, (key, e)) in unsafe { slab.iter_mut() }.enumerate() {
        assert_eq!(i, key);
        *e = *e + 1;
    }

    let vals: Vec<_> = slab.iter().map(|(_, r)| *r).collect();
    assert_eq!(vals, vec![1, 2, 3, 4]);

    slab.remove(2);

    for (_, e) in unsafe { slab.iter_mut() } {
        *e = *e + 1;
    }

    let vals: Vec<_> = slab.iter().map(|(_, r)| *r).collect();
    assert_eq!(vals, vec![2, 3, 5]);
}
