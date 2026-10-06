use bobot::{Bitboard16, Board, Symmetry};
use criterion::{Criterion, criterion_group, criterion_main};
use rand::{RngExt, SeedableRng as _};
use rapidrand::RapidRng;
use std::hint::black_box;

fn make_random_bitboard(seed: u64) -> Bitboard16 {
    let mut bb = Bitboard16::new();
    let mut rng = RapidRng::seed_from_u64(seed);
    for row in 0..Bitboard16::SIZE {
        for col in 0..Bitboard16::SIZE {
            if rng.random_range(0..=1) != 0 {
                bb.set(bobot::Position { row, col }, true);
            }
        }
    }
    bb
}

fn symmetries_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply_symmetry");
    let bb = make_random_bitboard(42);
    for (name, symmetry) in [
        (
            "hflip",
            Symmetry {
                hflip: true,
                ..Default::default()
            },
        ),
        (
            "vflip",
            Symmetry {
                vflip: true,
                ..Default::default()
            },
        ),
        (
            "transpose",
            Symmetry {
                transpose: true,
                ..Default::default()
            },
        ),
    ] {
        for board_size in [Bitboard16::SIZE, Board::SIZE] {
            let name = format!("{}-{}", name, board_size);
            let bb = bb & Bitboard16::board_mask(board_size);
            group.bench_with_input(
                name,
                &(symmetry, board_size),
                |b, (symmetry, board_size)| {
                    b.iter(|| black_box(bb).apply_symmetry(*symmetry, *board_size))
                },
            );
        }
    }
}

fn dilate_benchmark(c: &mut Criterion) {
    let bb = make_random_bitboard(43);

    c.bench_function("dilate", |b| b.iter(|| black_box(bb).dilate()));
}

criterion_group!(benches, symmetries_benchmark, dilate_benchmark);
criterion_main!(benches);
