/// Simple PRNG, based on
/// splitmix64.c by Sebastiano Vigna
/// https://xoshiro.di.unimi.it/splitmix64.c
/// Implemented with only const functions.
#[derive(Copy, Clone, Debug)]
pub struct Splitmix64(u64);

impl Splitmix64 {
    pub const fn with_seed(value: u64) -> Splitmix64 {
        Splitmix64(value)
    }

    pub const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;

        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod splitmix64 {
        use super::*;

        /// Tests that the results match the known values from Rosetta Code
        /// https://rosettacode.org/wiki/Pseudo-random_numbers/Splitmix64
        #[test]
        fn rosetta_code_reference_values() {
            let mut splitmix = Splitmix64::with_seed(1234567);

            assert_eq!(splitmix.next(), 6457827717110365317);
            assert_eq!(splitmix.next(), 3203168211198807973);
            assert_eq!(splitmix.next(), 9817491932198370423);
            assert_eq!(splitmix.next(), 4593380528125082431);
            assert_eq!(splitmix.next(), 16408922859458223821);
        }

        /// Tests five-binned histogram from Rosetta Code.
        /// https://rosettacode.org/wiki/Pseudo-random_numbers/Splitmix64
        #[test]
        fn rosetta_code_five() {
            let mut splitmix = Splitmix64::with_seed(987654321);
            let mut counts = [0usize; 5];

            for _i in 0..100_000 {
                let index = splitmix.next() / (((1u128 << 64) / 5) as u64);
                counts[index as usize] += 1;
            }

            assert_eq!(counts, [20027, 19892, 20073, 19978, 20030]);
        }
    }
}
