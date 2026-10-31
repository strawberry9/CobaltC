// Benchmark: the same workload as strings.cb, written the same way.
mod rng;
use rng::Rng;
use std::collections::HashMap;

fn main() {
    let mut words: Vec<String> = Vec::new();
    let mut rng = Rng::new(12345);
    for _i in 0..100000 {
        words.push(format!("w{}", rng.below(5000)));
    }
    let mut counts: HashMap<String, u32> = HashMap::new();
    for w in &words {
        *counts.entry(w.clone()).or_insert(0) += 1;
    }
    words.sort();
    let mut total = 0usize;
    for w in &words {
        total += w.len();
    }
    println!("{} distinct, {} bytes, first {}", counts.len(), total, words[0]);
}
