// Benchmark: the same two workloads as bench.cb, written the same way.

fn sieve_count(limit: usize) -> u64 {
    let mut is_prime: Vec<bool> = Vec::new();
    let mut i = 0;
    while i < limit {
        is_prime.push(i >= 2);
        i += 1;
    }
    let mut p = 2;
    while p * p < limit {
        if is_prime[p] {
            let mut m = p * p;
            while m < limit {
                is_prime[m] = false;
                m += p;
            }
        }
        p += 1;
    }
    let mut n = 0u64;
    let mut k = 0;
    while k < limit {
        if is_prime[k] {
            n += 1;
        }
        k += 1;
    }
    n
}

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
    println!("primes below 100000000: {}", sieve_count(100_000_000));
    let mut best = 0u64;
    let mut best_start = 0u64;
    let mut s = 1u64;
    while s < 1_000_000 {
        let len = collatz_len(s);
        if len > best {
            best = len;
            best_start = s;
        }
        s += 1;
    }
    println!("longest collatz chain below 1000000: start {}, length {}", best_start, best);
}
