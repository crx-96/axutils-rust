#[cfg(feature = "redis")]
use criterion::{criterion_group, criterion_main};

#[cfg(feature = "redis")]
mod enabled {
    use criterion::{Criterion, Throughput};
    use serde::{Deserialize, Serialize};
    use std::hint::black_box;

    #[derive(Debug, Serialize, Deserialize)]
    struct Payload {
        id: u64,
        name: String,
        groups: Vec<Group>,
        bytes: Vec<u8>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Group {
        label: String,
        values: Vec<u32>,
    }

    fn payload(size: usize) -> Payload {
        Payload {
            id: size as u64,
            name: format!("benchmark-{size}"),
            groups: (0..size.clamp(1, 32))
                .map(|group| Group {
                    label: format!("group-{group}"),
                    values: (0..size.clamp(1, 128) as u32)
                        .map(|value| value + group as u32)
                        .collect(),
                })
                .collect(),
            bytes: (0..size).map(|value| (value % 251) as u8).collect(),
        }
    }

    pub(crate) fn messagepack_encode_decode(criterion: &mut Criterion) {
        let mut group = criterion.benchmark_group("redis_messagepack");
        for size in [16, 256, 4096] {
            let payload = payload(size);
            let bytes = rmp_serde::to_vec(&payload).expect("encode benchmark payload");
            let encoded_size = bytes.len();
            let label = format!("{size}b_{encoded_size}out");
            group.throughput(Throughput::Bytes(encoded_size as u64));

            group.bench_function(format!("encode_{label}"), |bencher| {
                bencher.iter(|| {
                    let encoded = rmp_serde::to_vec(black_box(&payload)).expect("encode");
                    black_box(encoded);
                });
            });
            group.bench_function(format!("decode_{label}"), |bencher| {
                bencher.iter(|| {
                    let decoded: Payload =
                        rmp_serde::from_slice(black_box(&bytes)).expect("decode");
                    black_box(decoded);
                });
            });
        }
        group.finish();
    }
}

#[cfg(feature = "redis")]
criterion_group!(benches, enabled::messagepack_encode_decode);

#[cfg(feature = "redis")]
criterion_main!(benches);

#[cfg(not(feature = "redis"))]
fn main() {}
