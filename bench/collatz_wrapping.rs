// Benchmark: the same workload as collatz_wrapping.cb, written the same way.

fn collatz_len(start: u64) -> u64 {
    let mut x = start;
    let mut steps = 1u64;
    while x != 1 {
        if x % 2 == 0 {
            x /= 2;
        } else {
            x = 3u64.wrapping_mul(x).wrapping_add(1);
        }
        steps = steps.wrapping_add(1);
    }
    steps
}

fn main() {
    let mut best = 0u64;
    let mut s = 1u64;
    while s < 1_000_000 {
        let len = collatz_len(s);
        if len > best {
            best = len;
        }
        s += 1;
    }
    println!("{}", best);
}
