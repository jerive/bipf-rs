# bipf-rs

![Rust](https://github.com/jerive/bipf-rs/workflows/Rust/badge.svg)

Rust port of https://github.com/ssbc/bipf

TEST PROJECT

## Benchmark (Rust vs JS)

```
neon.binary.encode x 40,556 ops/sec ±0.69% (82 runs sampled)
binary.encode x 41,677 ops/sec ±0.38% (82 runs sampled)
neon.binary.decode x 72,686 ops/sec ±1.69% (82 runs sampled)
binary.decode x 71,403 ops/sec ±0.49% (82 runs sampled)
neon.binary.seek(string) x 651,465 ops/sec ±0.53% (85 runs sampled)
binary.seek(string) x 644,308 ops/sec ±1.26% (77 runs sampled)
neon.binary.seek(buffer) x 614,023 ops/sec ±1.03% (82 runs sampled)
binary.seek(buffer) x 1,068,828 ops/sec ±1.06% (79 runs sampled)

```
