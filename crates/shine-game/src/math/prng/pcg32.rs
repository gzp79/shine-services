use crate::math::prng::StableRng;

pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    pub fn new(seed: u64, seq: u64) -> Self {
        let mut pcg = Self { state: 0, inc: (seq << 1) | 1 };
        pcg.next();
        pcg.state = pcg.state.wrapping_add(seed);
        pcg.next();
        pcg
    }

    pub fn next(&mut self) -> u32 {
        let oldstate = self.state;
        self.state = oldstate.wrapping_mul(0x5851f42d4c957f2d).wrapping_add(self.inc);
        let xorshifted = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot = (oldstate >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Advance the state by delta steps, where delta can be negative to go backwards.
    pub fn advance(&mut self, delta: i64) {
        let mut cur_mult = 0x5851f42d4c957f2d;
        let mut cur_plus = self.inc;
        let mut acc_mult: u64 = 1;
        let mut acc_plus: u64 = 0;

        // Even though delta is an unsigned integer, we can pass a signed
        // integer to go backwards, it just goes "the long way round".
        let mut delta = delta as u64;

        while delta > 0 {
            if (delta & 1) != 0 {
                acc_mult = acc_mult.wrapping_mul(cur_mult);
                acc_plus = acc_plus.wrapping_mul(cur_mult).wrapping_add(cur_plus);
            }
            cur_plus = cur_mult.wrapping_add(1).wrapping_mul(cur_plus);
            cur_mult = cur_mult.wrapping_mul(cur_mult);
            delta /= 2;
        }
        self.state = acc_mult.wrapping_mul(self.state).wrapping_add(acc_plus);
    }
}

impl StableRng for Pcg32 {
    fn next_u32(&mut self) -> u32 {
        self.next()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use shine_test::test;

    #[test]
    fn test_pcg32() {
        let mut rng = Pcg32::new(42, 54);
        let expected = [0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e];
        for &exp in &expected {
            assert_eq!(rng.next(), exp);
        }
    }

    #[test]
    fn test_pcg32_advance() {
        {
            let mut rng = Pcg32::new(42, 54);
            rng.advance(5);
            assert_eq!(rng.next(), 0xcbed606e);
            rng.advance(-3);
            assert_eq!(rng.next(), 0x83d2f293);
        }

        {
            let mut rng = Pcg32::new(42, 54);
            let mut rng2 = Pcg32::new(42, 54);
            let _r0 = rng.next(); // 0xa15c02b7
            let r1 = rng.next(); // 0x7b47f409
            let r2 = rng.next(); // 0xba1d3330
            let r3 = rng.next(); // 0x83d2f293
            rng2.advance(3);
            assert_eq!(r3, rng2.next());

            // repeat last random
            rng2.advance(-1);
            assert_eq!(r3, rng2.next());
            rng2.advance(-1);
            assert_eq!(r3, rng2.next());

            rng2.advance(-2);
            assert_eq!(rng2.next(), r2);
            rng2.advance(-2);
            assert_eq!(rng2.next(), r1);
        }

        for test in [7, 13, 57, 1001] {
            let mut rng = Pcg32::new(42, 54);
            for _ in 0..test {
                rng.next();
            }
            let mut rng2 = Pcg32::new(42, 54);
            rng2.advance(test);
            assert_eq!(rng.next(), rng2.next()); // (test + 1). in the sequence

            // go back the the 1th random
            // test+1 backward step is required to reset the stream
            rng2.advance(-test);
            assert_eq!(rng2.next(), 0x7b47f409);
        }
    }
}
