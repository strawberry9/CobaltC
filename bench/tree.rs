// Benchmark: the same workload as tree.cb, written the same way.
mod rng;
use rng::Rng;

struct Node {
    key: i64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

fn insert(n: &mut Node, k: i64) {
    if k < n.key {
        match &mut n.left {
            Some(b) => insert(b, k),
            None => n.left = Some(Box::new(Node { key: k, left: None, right: None })),
        }
    } else {
        match &mut n.right {
            Some(b) => insert(b, k),
            None => n.right = Some(Box::new(Node { key: k, left: None, right: None })),
        }
    }
}

fn sum(n: &Node) -> i64 {
    let mut s = n.key;
    if let Some(b) = &n.left {
        s += sum(b);
    }
    if let Some(b) = &n.right {
        s += sum(b);
    }
    s
}

fn main() {
    let mut root = Node { key: 500000, left: None, right: None };
    let mut rng = Rng::new(99);
    for _i in 0..60000 {
        insert(&mut root, rng.below(1000000) as i64);
    }
    println!("sum {}", sum(&root));
}
