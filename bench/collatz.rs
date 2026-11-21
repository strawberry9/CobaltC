// Benchmark: the same workload as collatz.cb, written the same way.

fn collatz_len(start: u64) -> u64 {
    let mut x = start;
    let mut steps = 1u64;
    while x != 1 {
        if x % 2 == 0 {
            x /= 2;
        } else {
            x = 3 * x + 1;
        }
        steps += 1;
    }
    steps
}

fn main() {
    let mut best = 0u64;
    for s in 1..1_000_000u64 {
        let len = collatz_len(s);
        if len > best {
            best = len;
        }
    }
    println!("{}", best);
}
