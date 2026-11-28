// Benchmark: the same workload as hashmap.cb, written the same way.
mod rng;
use rng::Rng;
use std::collections::HashMap;

fn shuffle<T>(v: &mut Vec<T>, rng: &mut Rng) {
    let mut i = v.len();
    while i > 1 {
        let j = rng.below(i as u64) as usize;
        i -= 1;
        v.swap(i, j);
    }
}

fn texts(n: usize, seed: u64, prefix: &str) -> Vec<String> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(format!("{}{}", prefix, rng.next_u64() >> 20));
    }
    out
}

fn numbers() -> u64 {
    let n = 1000000usize;
    let mut rng = Rng::new(2026);
    let mut keys: Vec<u64> = Vec::new();
    let mut absent: Vec<u64> = Vec::new();
    for _ in 0..n {
        keys.push(rng.next_u64() | 1);
        absent.push((rng.next_u64() >> 1) << 1);
    }
    let mut order = keys.clone();
    shuffle(&mut order, &mut rng);

    let mut m: HashMap<u64, u64> = HashMap::new();
    let mut next = 0u64;
    for k in &keys {
        m.insert(*k, next);
        next += 1;
    }
    let mut sum = 0u64;
    for _pass in 0..2 {
        for k in &order {
            if let Some(v) = m.get(k) {
                sum = sum.wrapping_add(*v);
            }
        }
    }
    for k in &absent {
        if m.contains_key(k) {
            sum = sum.wrapping_add(1);
        }
    }
    for k in &order {
        *m.entry(*k).or_insert(0) += 1;
    }
    for i in 0..n / 2 {
        if let Some(v) = m.remove(&order[2 * i]) {
            sum = sum.wrapping_add(v);
        }
    }
    for (k, v) in &m {
        sum = sum.wrapping_add(k.wrapping_add(*v));
    }
    sum.wrapping_add(m.len() as u64)
}

fn words() -> u64 {
    let n = 200000usize;
    let mut rng = Rng::new(7);
    let mut order = texts(n, 11, "key_");
    shuffle(&mut order, &mut rng);
    let absent = texts(n, 13, "abs_");

    let mut m: HashMap<String, u64> = HashMap::new();
    let mut next = 0u64;
    for k in texts(n, 11, "key_") {
        m.insert(k, next);
        next += 1;
    }
    let mut sum = 0u64;
    for _pass in 0..2 {
        for k in &order {
            if let Some(v) = m.get(k.as_str()) {
                sum = sum.wrapping_add(*v);
            }
        }
    }
    for k in &absent {
        if m.contains_key(k.as_str()) {
            sum = sum.wrapping_add(1);
        }
    }
    sum.wrapping_add(m.len() as u64)
}

fn main() {
    println!("{} {}", numbers(), words());
}
