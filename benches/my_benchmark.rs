use criterion::{black_box, criterion_group, criterion_main, Criterion};

use bipf_rs::*;
use serde_json::json;

fn criterion_benchmark(c: &mut Criterion) {
    let json = json!({
      "name": "bipf",
      "description": "binary in-place format",
      "version": "1.5.1",
      "homepage": "https://github.com/ssbc/bipf",
      "repository": {
        "type": "git",
        "url": "git://github.com/ssbc/bipf.git"
      },
      "dependencies": {
        "varint": "^5.0.0"
      },
      "devDependencies": {
        "faker": "^5.5.1",
        "tape": "^4.9.0"
      },
      "scripts": {
        "test": "node test/index.js && node test/compare.js"
      },
      "author": "Dominic Tarr <dominic.tarr@gmail.com> (http://dominictarr.com)",
      "license": "MIT"
    });
    let serialized = json.to_bipf().unwrap();
    let json_string = json.to_string();
    let json_bytes = json_string.as_bytes();
    c.bench_function("binary.encode", |b| {
        b.iter(|| black_box(json.clone().to_bipf()))
    });
    c.bench_function("binary.decode", |b| {
        b.iter(|| decode(black_box(&serialized)))
    });
    c.bench_function("serde_json.parse", |b| {
        b.iter(|| {
            serde_json::to_value(black_box(json_bytes)).unwrap();
        })
    });
    c.bench_function("serde_json.stringify", |b| {
        b.iter(|| {
            black_box(&json).to_string();
        })
    });
    c.bench_function("binary.seek", |b| {
        b.iter(|| {
            black_box({
                let k = seek_key(&serialized, Some(0), String::from("dependencies")).unwrap();
                let s = seek_key(&serialized, Some(k), String::from("varint")).unwrap();
                decode_rec(&serialized, s)
            })
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
