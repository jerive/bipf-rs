use criterion::{black_box, criterion_group, criterion_main, Criterion};

use bipf_rs::bipf::*;
use serde_json::json;
use serde_json::Value;

fn test(o: Value) {
    o.to_bipf();
}

fn criterion_benchmark(c: &mut Criterion) {
    let json = json!({ "hello": 10000 });
    let serialized = json.to_bipf();
    c.bench_function("serialization simple", |b| b.iter(|| test(black_box(json.clone()))));
    c.bench_function("deserialization simple", |b| b.iter(|| decode(black_box(&serialized.clone()))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);