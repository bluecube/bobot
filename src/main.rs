mod bitboard;
mod board;
mod color;

fn main() {
    #[allow(unused_assignments)]
    let bb = bitboard::Bitboard16::new();

    // dbg!(bb.to_ascii(13));
    let bit = std::hint::black_box(std::hint::black_box(&bb).arbitrary_set_bit());
    // dbg!(bit.to_ascii(13));
}
