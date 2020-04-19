# Slab

[![crates.io](https://img.shields.io/badge/crates.io-v0.1.0-orange)](https://crates.io/crates/pinned_slab)

Slab-allocator with pinned elements.

Much of this code is directly taken from
[`slab`](https://github.com/carllerche/slab) and should have roughly the same
interface. If you see a function missing that you'd like implemented or a
feature flag to configure `CHUNK_SIZE` through const generics, pull requests
are welcome!

## Usage

For now, you should see [the documentation for `slab`](https://docs.rs/slab/0.4.2/slab/)
for general usage.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `pinned_slab` by you, shall be licensed as MIT, without any additional
terms or conditions.
