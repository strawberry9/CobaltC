// Benchmark: the same workload as hashcount.cb, written the same way.
mod rng;
use rng::Rng;
use std::collections::HashMap;

fn main() {
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut rng = Rng::new(12345);
    for _i in 0..200000 {
        *counts.entry(format!("w{}", rng.below(5000))).or_insert(0) += 1;
    }
    println!("{}", counts.len());
}
