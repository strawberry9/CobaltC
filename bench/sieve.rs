// Benchmark: the same workload as sieve.cb, written the same way.

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

fn main() {
    println!("{}", sieve_count(100_000_000));
}
