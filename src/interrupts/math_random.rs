use crate::{Storage, VirtMach, VMAtom, VMUAtom};

/// The state of the generator, xorshift32: shifts and xors only, no multiplication or division,
/// which an 8 bit cpu does not have
static mut SEED: u32 = 0;
/// Once the program set a seed, the sequence only depends on it and not on the cycle count
static mut SEEDED: bool = false;

fn next(salt: u32) -> VMUAtom {
    let mut state = unsafe { SEED } ^ salt;
    if state == 0 {
        state = 0x9e37_79b9;
    }
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    unsafe { SEED = state; }
    // the high bits are the best ones
    (state >> (32 - VMUAtom::BITS)) as VMUAtom
}

/// Calls the random function of the math interrupt, 11: a random value from start to end, both
/// included, in either order (start,end)->(res). All of it is done in the size of an atom.
pub fn call<S: Storage>(vm: &mut VirtMach<S>) {
    let (a, b) = (vm.stack_pop(), vm.stack_pop());
    let (min, max) = if a <= b { (a, b) } else { (b, a) };
    // one less than the number of values, so the full range fits
    let span = max.wrapping_sub(min) as VMUAtom;
    let salt = if unsafe { SEEDED } { 0 } else { (vm.cycle_cnt as u32) ^ (vm.program.id.as_ptr() as usize as u32) };
    let random = next(salt);
    let offset = if span == VMUAtom::MAX { random } else { random % (span + 1) };
    let res = min.wrapping_add(offset as VMAtom);
    vm.processor.zero = res == 0;
    vm.processor.carry = false;
    vm.stack_push(res);
}

/// Calls the seed function of the math interrupt, 12: starts the same sequence of random values
/// again for the same seed (seed)->(). Without it the values also depend on when they are asked for.
pub fn seed<S: Storage>(vm: &mut VirtMach<S>) {
    let seed = vm.stack_pop() as VMUAtom;
    // the seed is spread over the state so that small seeds do not start with similar values
    unsafe { SEED = (seed as u32).wrapping_mul(0x9e37_79b9) ^ seed as u32; SEEDED = true; }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the generator is global, the tests take turns
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn random(a: VMAtom, b: VMAtom) -> VMAtom {
        let mut vm = VirtMach::new();
        vm.stack_push(b);
        vm.stack_push(a);
        call(&mut vm);
        vm.stack_pop()
    }

    #[test]
    fn stays_in_range() {
        let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for (a, b) in [(1, 6), (6, 1), (-3, 3), (0, 0), (5, 5), (VMAtom::MIN, VMAtom::MAX), (VMAtom::MAX, VMAtom::MIN), (VMAtom::MAX - 1, VMAtom::MAX)] {
            for _ in 0..500 {
                let res = random(a, b);
                assert!(res >= a.min(b) && res <= a.max(b), "{} {} gave {}", a, b, res);
            }
        }
    }

    #[test]
    fn seed_repeats_the_sequence() {
        let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        fn seeded(seed: VMAtom) -> [VMAtom; 8] {
            let mut vm = VirtMach::new();
            vm.stack_push(seed);
            super::seed(&mut vm);
            let mut values = [0; 8];
            for v in values.iter_mut() { vm.cycle_cnt += 7; *v = random(0, 100); }
            values
        }
        // the generator is global, so this is one test
        assert_eq!(seeded(42), seeded(42));
        assert_ne!(seeded(42), seeded(43));
    }

    #[test]
    fn reaches_all_values() {
        let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut seen = [false; 6];
        for _ in 0..500 {
            seen[(random(1, 6) - 1) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "{:?}", seen);
    }
}
