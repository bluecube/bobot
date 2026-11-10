use bobot::{Board, Color, ZobristHash, solver::random_rollout::play_random_rollout};
use criterion::{Criterion, criterion_group, criterion_main};
use rand::SeedableRng as _;
use rapidrand::RapidRng;
use std::hint::black_box;

fn random_rollout_benchmark(c: &mut Criterion) {
    let mut rng = RapidRng::seed_from_u64(42);
    c.bench_function("random_rollout", |b| {
        b.iter(|| {
            play_random_rollout(
                black_box(Board::new()),
                black_box(Color::Black),
                black_box(0),
                black_box(ZobristHash::default()),
                black_box(3 * Board::AREA as usize),
                black_box(&mut rng),
            )
        })
    });
}

criterion_group!(benches, random_rollout_benchmark);
criterion_main!(benches);
