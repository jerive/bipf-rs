# bipf-rs

![Rust](https://github.com/jerive/bipf-rs/workflows/Rust/badge.svg)

Rust port of https://github.com/ssbc/bipf

## Benchmark (Rust vs JS)

### JS 

as described in https://npm.io/package/@staltz/bipf

```
operation, ops/ms

binary.encode 96.15384615384616
JSON.stringify 555.5555555555555
binary.decode 208.33333333333334
JSON.parse 476.1904761904762
JSON.parse(buffer) 416.6666666666667
JSON.stringify(JSON.parse()) 250
binary.seek(string) 714.2857142857143
binary.seek2(encoded) 1250
binary.seek(buffer) 2000
binary.seekPath(encoded) 769.2307692307693
binary.seekPath(compiled) 1666.6666666666667
binary.compare() 1666.6666666666667

```

### Rust

```
operation, ops/ms

binary.encode 190 (x2)
binary.decode 450 (x2)
binary.seek seek 7700 (x4)
```