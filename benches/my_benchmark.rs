use criterion::{black_box, criterion_group, criterion_main, Criterion};

use bipf_rs::bipf::*;
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
    let serialized = json.to_bipf();
    c.bench_function("serialization simple", |b| b.iter(|| black_box(json.clone().to_bipf())));
    c.bench_function("deserialization simple", |b| b.iter(|| decode(black_box(&serialized.clone()))));
    c.bench_function("seek key", |b| b.iter(|| black_box({
      let k = seek_key(&serialized, Some(0), String::from("dependencies")).unwrap();
      let s = seek_key(&serialized, Some(k), String::from("varint")).unwrap();
      decode_rec(&serialized, s)
    })));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
