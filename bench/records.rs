// Benchmark: the same workload as records.cb, written the same way.
struct Particle {
    x: i64,
    y: i64,
    vx: i64,
    vy: i64,
}

fn step(p: &mut Particle) {
    p.x += p.vx;
    p.y += p.vy;
    if p.x < 0 || p.x > 10000 {
        p.vx = -p.vx;
    }
    if p.y < 0 || p.y > 10000 {
        p.vy = -p.vy;
    }
}

fn energy(ps: &Vec<Particle>) -> i64 {
    let mut e = 0i64;
    for p in ps {
        e += p.vx * p.vx + p.vy * p.vy;
    }
    e
}

fn main() {
    let mut ps: Vec<Particle> = Vec::new();
    for k in 0..2000i64 {
        ps.push(Particle { x: k * 5, y: k * 3, vx: k % 7 - 3, vy: k % 5 - 2 });
    }
    for _t in 0..500 {
        for i in 0..ps.len() {
            step(&mut ps[i]);
        }
    }
    println!("energy {}, first at {} {}", energy(&ps), ps[0].x, ps[0].y);
}
