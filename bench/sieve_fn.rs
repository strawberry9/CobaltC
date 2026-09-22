// Benchmark: the same workload as sieve_fn.cb, written the same way.

fn fill(is_prime: &mut Vec<bool>, limit: usize) {
    let mut i = 0;
    while i < limit {
        is_prime.push(i >= 2);
        i += 1;
    }
}

fn mark(is_prime: &mut Vec<bool>, limit: usize) {
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
}

fn count(is_prime: &Vec<bool>) -> u64 {
    let mut n = 0u64;
    let mut k = 0;
    while k < is_prime.len() {
        if is_prime[k] {
            n += 1;
        }
        k += 1;
    }
    n
}

fn main() {
    let limit = 100_000_000;
    let mut is_prime: Vec<bool> = Vec::new();
    fill(&mut is_prime, limit);
    mark(&mut is_prime, limit);
    println!("{}", count(&is_prime));
}
