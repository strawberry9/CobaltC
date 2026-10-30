# Threads

CobaltC's concurrency is small on purpose. You start a thread and you must
wait for it. Data goes to a thread by moving, by lending a reference, or
through a channel; data shared by several threads lives in a mutex or an
atomic. And the rules of chapter 7 apply unchanged across threads, so a
data race is an aliasing conflict like any other, reported rather than
suffered. This chapter is about those few pieces and the guarantees around
them.

## `spawn` and `join`

```
import std;

fn sum_squares(u64 from, u64 to) : u64
{
    u64 total = 0;
    for (u64 i = from; i < to; i += 1)
    {
        total += i * i;
    }
    total
}

fn main()
{
    handle<u64> low = spawn(sum_squares, 0, 500);       // runs on its own thread
    handle<u64> high = spawn(sum_squares, 500, 1000);   // and so does this
    u64 total = join(low) + join(high);                  // wait for each, take its result
    printf("%v\n", total);
    printf("%v\n", sum_squares(0, 1000));                // the same, done here
}
```

```
332833500
332833500
```

`spawn(f, args...)` runs `f(args...)` on a new thread and returns a
`handle<R>`, where `R` is what `f` returns. `join(h)` waits for the thread
and gives you its result. A handle is a resource: if it goes out of scope
without being joined, the thread is joined then and its result discarded, so
a thread never outlives the scope that owns its handle. That one rule is
what makes everything else in this chapter safe: a reference lent to a
thread is a borrow that ends at the join, which is always before the lender's
scope ends.

The compiler runs threads in parallel on separate cores. The interpreter
runs them one at a time, taking turns. A program that follows the rules gets
the same result from both.

## Giving a thread its data

Arguments to `spawn` are passed exactly as to a call: plain values are
copied, resources move, and references are lent. Moving is the simplest:

```
import std;

fn longest(Vec<String> words) : String        // takes the vector: it moves to the thread
{
    String best = String::new();
    foreach (w in words)
    {
        if (String::len(&w) > String::len(&best))
        {
            best = w;
        }
    }
    best
}

fn main()
{
    Vec<String> words = Vec::new();
    Vec::push(&mut words, "pear");
    Vec::push(&mut words, "banana");
    Vec::push(&mut words, "fig");
    handle<String> h = spawn(longest, words);   // words is gone from here
    String result = join(h);                    // and the String comes back
    printf("%v\n", &result);
}
```

```
banana
```

The vector moves into the thread, which owns it, and the result moves back
through `join`. Nothing is shared, so nothing needs a lock. A `move` closure
does the same job when the thread's code is better written inline:
`spawn(move [words]() { ... })`. A closure that merely borrows locals cannot
be spawned, since the thread might outlive them.

Lending a reference works too, and the borrow lasts until the join:

```
import std;

fn count_over(ref<Vec<i32>, shared> v, i32 limit) : usize
{
    usize n = 0;
    foreach (x in v)
    {
        if (*x > limit)
        {
            n += 1;
        }
    }
    n
}

fn main()
{
    Vec<i32> v = Vec::from_slice(&[5, 12, 7, 30, 1][0..$]);
    handle<usize> h = spawn(count_over, &v, 6);    // the thread borrows v until join
    Vec::push(&mut v, 99);            // ✗ the thread may still be reading v
    printf("%v\n", join(h));
}
```

```
error[diag.aliasing-conflict]: `h` still borrows `v`, so it cannot be written or borrowed `mut` here; end that borrow first
  --> borrow_until_join.cb:20:15
   |
20 |     Vec::push(&mut v, 99);            // ✗ the thread may still be reading v
   |               ^
   = repair: end the conflicting reference (let its holder go out of scope) before this access, or use `shared` on both sides
```

Whether the thread had finished reading before the push is a matter of
timing, and a program whose correctness depends on timing is rejected
instead. Move the push after the `join` and the program is accepted. The
same checker that stops a reference being kept across a `push` in one
thread stops it across two; there is no separate theory of data races.

## Sharing state: `mutex`

When several threads must change one value, it goes in a `mutex`, and the
only way to reach the value is to lock it:

```
import std;

fn worker(ref<mutex<u64>, shared> total, u64 n)
{
    foreach (i in 1..n + 1)
    {
        guard<u64> g = lock(total);   // waits until the mutex is free
        *g += i;
    }                                 // the guard ends here, unlocking
}

fn main()
{
    mutex<u64> total = Mutex::new(0: u64);
    {
        handle<void> a = spawn(worker, &total, 100);
        handle<void> b = spawn(worker, &total, 100);
    }                                 // both handles end here: both threads are joined
    printf("%v\n", *lock(&total));    // 2 * (1 + 2 + ... + 100)
}
```

```
10100
```

`lock(&m)` returns a `guard<T>`, an exclusive reference into the mutex that
exists as long as the guard does. `*g` is the value; `g.field` reaches a
field of it. The guard is a resource, and its destruction unlocks the mutex,
so the lock is held for exactly the guard's scope and cannot be forgotten.
Another thread's `lock` waits. A thread that locks a mutex it already holds
faults rather than deadlocking.

A mutex is shared by passing `&m`, a shared reference, to any number of
threads. It is the one place an exclusive path, the guard, is allowed to
coexist with shared references to the same object, because the lock
serialises the accesses. Keep each guard in the smallest block that needs
it; a guard held across a long computation is a lock held that long.

For a single integer shared by threads, a counter, a flag, a running maximum,
`Atomic<T>` from the library does the job without a lock: `Atomic::fetch_add`,
`Atomic::load`, `Atomic::compare_exchange` and the rest, each indivisible and
each sequentially consistent. State of more than one value still belongs in
a mutex.

## Passing values: `Channel`

A channel is a queue between threads. Values move in at one end and out at
the other, so a `String` or a `Vec` travels between threads with one owner
at every moment:

```
import std;

fn worker(ref<Channel<u64>, shared> jobs, ref<Channel<u64>, shared> results)
{
    while (Some(n) = Channel::recv(jobs))       // None once the channel is closed and empty
    {
        _ = Channel::send(results, n * n);
    }
}

fn main()
{
    Channel<u64> jobs = Channel::new(4);         // holds at most 4 waiting jobs
    Channel<u64> results = Channel::new(100);
    {
        handle<void> a = spawn(worker, &jobs, &results);
        handle<void> b = spawn(worker, &jobs, &results);
        foreach (i in 1..11: u64)
        {
            _ = Channel::send(&jobs, i);         // waits while the channel is full
        }
        Channel::close(&jobs);                   // the workers stop when it runs dry
    }                                            // and are joined here
    Channel::close(&results);
    u64 total = 0;
    while (Some(sq) = Channel::recv(&results))
    {
        total += sq;
    }
    printf("%v\n", total);                       // 1 + 4 + ... + 100, in whatever order they came
}
```

```
385
```

`send` waits while the channel is full and `recv` while it is empty; a
waiting thread uses no processor time. `close` says nothing more is coming:
what was sent is still received, and then `recv` gives `None`, which ends a
worker's loop. This shape, a pool of workers fed by one channel and closed
when the work is done, is the common one. `send` returns a `Result`, since a
value sent after `close` comes back as an error rather than vanishing; the
`_ =` says the result is not needed here.

## When a thread fails

A fault in a thread you started ends that thread, not the program. The
thread unwinds, its destructors run, a mutex it held is poisoned so that the
next `lock` of it faults, and its failure becomes a value at the boundary:

```
import std;

fn handler(usize i) : u64
{
    Vec<u64> v = Vec::filled(3, 7: u64);
    v[i]                              // a bug when i >= 3
}

fn main()
{
    usize served = 0;
    usize failed = 0;
    foreach (i in 0..5)
    {
        match (try_join(spawn(handler, i)))     // the thread's fault, as a value
        {
            Ok(_)  : served += 1,
            Err(f) :
            {
                printf("thread %v failed: %v\n", i, &ThreadFailure::diagnostic(&f));
                failed += 1;
            },
        }
    }
    printf("%v served, %v failed\n", served, failed);
}
```

```
thread 3 failed: diag.index-out-of-bounds
thread 4 failed: diag.index-out-of-bounds
3 served, 2 failed
```

`try_join(h)` returns `Result<R, ThreadFailure>`, with the failure's
diagnostic name and its full report available as text. A plain `join` of a
failed thread raises the failure again in the joiner, and so does a handle
that ends unjoined, so a program that never looks still fails as a whole,
one scope at a time, rather than carrying on in a broken state. This is what
lets a server give each connection a thread and survive a bug in one of
them.

One exception keeps the guarantee honest. A thread that was lent an
exclusive reference may be halfway through changing data that belongs to a
thread still running, so its fault ends the program at once, as a fault
always did. Lend a thread exclusive access only when its failure should be
the program's; otherwise move the data in.

`cancel(&h)` asks a thread to stop: its current or next wait ends with a
contained failure, which `ThreadFailure::is_cancelled` recognises. And a
wait that can never end, every thread waiting on another, is reported as
`diag.deadlock` rather than hanging.

## Finding the timing bugs

A program whose result depends on which thread ran first is easy to write
and hard to notice, because the timing you test is rarely the one that goes
wrong. `coby --explore N prog.cb` runs a program `N` times under different
orders of its threads and groups the runs by outcome; more than one outcome
means the result depends on timing. Here is a program with a lost update:

```
import std;

fn bump(ref<mutex<i64>, shared> m)
{
    i64 seen = 0;
    {
        guard<i64> g = lock(m);
        seen = *g;
    }                                 // the lock is released here...
    guard<i64> g = lock(m);
    *g = seen + 1;                    // ...so the other thread may have run in between
}

fn main()
{
    mutex<i64> m = Mutex::new(0: i64);
    {
        handle<void> a = spawn(bump, &m);
        handle<void> b = spawn(bump, &m);
    }
    printf("total %v\n", *lock(&m));
}
```

```
explored 40 schedules of explore.cb: 2 distinct outcomes

15 runs, exit 0 -- replay: coby --schedule-seed 1 explore.cb
  | total 1

25 runs, exit 0 -- replay: coby --schedule-seed 2 explore.cb
  | total 2
```

Each outcome comes with a seed, and `coby --schedule-seed S prog.cb`
replays that run exactly, as often as needed to see why. `cobc --explore`
does the same with the compiled program. The fix here is to hold the lock
across the read and the write, which is the rule of thumb for every mutex:
a read-then-write under one lock is one operation, and under two it is
none.

## What is not here

There are no memory orderings weaker than sequential consistency, no
condition variables, no thread-local storage and no read-write lock. There
are, in the library, `Barrier`, `Semaphore` and `Once` for the common waiting
patterns, `Atomic::wait` and `notify` for sleeping until a value changes,
`Arc<T>` for a value that several threads own together, and `slice_parts`
for splitting a vector into parts that threads fill at the same time with no
lock, since disjoint slices never conflict. The language guide's chapter on
concurrency covers each, with its rules.
