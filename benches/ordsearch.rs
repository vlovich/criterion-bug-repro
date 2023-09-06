use std::hint::black_box;

use criterion::{
    criterion_group, criterion_main, measurement::Measurement, AxisScale, BenchmarkGroup,
    BenchmarkId, Criterion, PlotConfiguration,
};
use rand::{seq::SliceRandom, thread_rng, Rng};

const EXCEEDS_CPU_CACHE_LENGTH: usize = 92 * 1024 * 1024;

#[derive(Debug)]
enum KeySizeType {
    Fixed(usize),
}

enum KeySizeGen {
    Fixed(usize),
}

pub struct OrdsearchLookup<'a> {
    keys: ordsearch::OrderedCollection<&'a [u8]>,
}

impl<'a> OrdsearchLookup<'a> {
    fn new(keys: &[&'a [u8]]) -> Self {
        let mut keys: Vec<&[u8]> = keys.into_iter().copied().collect();
        keys.sort_unstable();
        Self {
            keys: ordsearch::OrderedCollection::from_sorted_iter(keys.iter().copied()),
        }
    }

    fn contains(&self, key: &[u8]) -> bool {
        if let Some(k) = self.keys.find_gte(key) {
            *k == key
        } else {
            eprintln!("Key not found");
            false
        }
    }
}

fn lookup_benchmark<'b, M>(
    keys: &[&'b [u8]],
    group: &mut BenchmarkGroup<'_, M>,
    bench_entry: (&str, OrdsearchLookup<'b>),
    key_size: KeySizeType,
) where
    M: Measurement,
{
    debug_assert!(keys.len() > 0);

    let (algorithm, key_lookup) = bench_entry;

    let num_keys = keys.len();

    group.throughput(criterion::Throughput::Elements(num_keys as u64));

    group.bench_with_input(
        BenchmarkId::new(
            algorithm,
            match key_size {
                KeySizeType::Fixed(key_size) => format!("{}/{}", num_keys, key_size),
            },
        ),
        &keys.len(),
        |b, &_| {
            b.iter(|| {
                let start = std::time::Instant::now();
                for key in keys.iter().copied() {
                    black_box(key_lookup.contains(key));
                }
                let elapsed = start.elapsed();
                eprintln!(
                    "\nTook {:?} or {} Kelements / s",
                    elapsed,
                    keys.len() as f64 / (1000f64 * elapsed.as_secs_f64())
                );
            });
        },
    );
}

fn make_keys<'a>(key_buffer: &'a [u8], mut key_size: KeySizeGen) -> Vec<&'a [u8]> {
    let mut offset = 0;
    let mut total_size_of_keys = 0;
    let max_keys = 5_000_000;
    let mut total_num_keys = 0;
    let mut keys: Vec<&[u8]> = std::iter::from_fn(|| {
        let key_size = match &mut key_size {
            KeySizeGen::Fixed(key_size) => *key_size,
        };

        if total_num_keys >= max_keys + 10_000 || total_size_of_keys + key_size > key_buffer.len() {
            return None;
        }

        total_size_of_keys += key_size;
        total_num_keys += 1;

        let key = unsafe { key_buffer.get_unchecked(offset..offset + key_size) };
        offset = (offset + key_size) % EXCEEDS_CPU_CACHE_LENGTH;
        return Some(key);
    })
    .collect();
    keys.sort_unstable();
    keys.dedup();
    if keys.len() > max_keys {
        keys.drain(max_keys..);
    }
    keys.shuffle(&mut thread_rng());
    keys
}

fn benchmark_lookups(c: &mut Criterion) {
    // From testing, filters take roughly the same amount of time to create regardless of the fingerprint size.
    // So no need to test each one to compare against other mechanisms.
    let mut rng = rand::thread_rng();

    let key_buffer =
        Vec::from_iter(std::iter::from_fn(|| Some(rng.gen::<u8>())).take(EXCEEDS_CPU_CACHE_LENGTH));

    let mut group = c.benchmark_group("Lookup Filter Fixed Keys");

    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for key_size in [5, 11, 20, 30] {
        let keys = make_keys(&key_buffer, KeySizeGen::Fixed(key_size));

        let bench_entry = ("ordsearch", OrdsearchLookup::new(&keys));

        lookup_benchmark(
            keys.as_slice(),
            &mut group,
            bench_entry,
            KeySizeType::Fixed(key_size),
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_lookups);
criterion_main!(benches);
