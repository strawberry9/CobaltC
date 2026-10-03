# D-0063 — Channels

Status: ACCEPTED (2026-09-27, owner-chosen)
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md` §1, §9, §17
Depends on: D-0016, D-0018, D-0019, rule.conc.spawn, rule.conc.lock
Affects: rule.conc.channel (new), rule.conc.lock (`sync-exempt`),
rule.stdlib.channel (new), rule.stdlib.prelude

## Problem

Threads could share state only by locking it. A thread that needed a
value another thread would produce had nothing to wait on but the
lock itself: it took the lock, found nothing, released it and took it
again, burning a processor. Under `coby`'s scheduler such a loop
starves the producer outright (the consumer re-takes the lock before
the waiting thread next looks). A pipeline or a pool of workers — the
common shape of a threaded program — could not be written well.

Writing the channel also exposed a gap in `sync-exempt` (`spec/19`
§2): a lock path was exempt only from shared references whose type is
exactly `mutex<_>`, so two threads each holding `&hub` to a struct with
a mutex in it clashed as soon as one of them locked it.

## Constraints

- No new tokens or syntax; the interpreter and the compiler alike;
  `coby` portable to Windows.
- The resource model unchanged: a value in a channel has one owner,
  the channel; nothing is copied.
- Waiting is blocking, not failure, as for `lock` and `join`.

## Candidate mechanisms

The type:

1. **`Channel<T>`, a resource type of `std` written in CobaltC over a
   `mutex` and one waiting primitive private to `std`.** **Selected.**
2. A built-in type with its own rules, as `mutex<τ>` is.
3. A condition variable exposed to programs, channels left to them.

Capacity:

4. **Bounded only: `Channel::new(cap)`, `cap` at least 1.** **Selected.**
5. Unbounded only.
6. Both, and a capacity of 0 meaning a rendezvous.

Closing:

7. **An explicit `Channel::close`.** **Selected.**
8. Closing when the last sender goes (sender handles counted).

Non-blocking operations:

9. **None for now.** **Selected.**
10. `try_send`, `try_recv`.

## Selected design

Candidates 1, 4, 7 and 9.

    Channel<T> Channel::new<T>(usize cap)                 -- cap = 0 faults
    Channel::send<T>(ref<Channel<T>, shared> ch, T v) : Result<void, T>
    Channel::recv<T>(ref<Channel<T>, shared> ch) : Option<T>
    Channel::close<T>(ref<Channel<T>, shared> ch)

- **Sharing.** Threads share a channel by `ref<Channel<T>, shared>`, as
  they share a mutex; a thread cannot outlive its handle, so it cannot
  outlive the channel.
- **`send`** waits while the channel is full, and gives the value back,
  `Err(v)`, once the channel is closed: nothing is lost.
- **`recv`** waits while the channel is empty, and gives `None` once it
  is closed and empty, so a receiver's loop ends with a `match`.
- **`close`** is idempotent. Values already sent are still received.
- **Destroying** a channel destroys the values still in it, oldest
  first.
- **Faults.** `Channel::new(0)` is `diag.channel-zero-capacity`. A
  wait by the program's only running thread (the main thread, no
  spawned thread running) can never end: `diag.channel-deadlock`.
  Other deadlocks remain out of scope (`spec/22` §5).
- **The waiting primitive** is an event count, private to `std`; a
  channel has two, one for waiting senders and one for waiting
  receivers. A change to a channel's state adds one to the count of the
  threads it may let proceed, under the channel's lock; a waiter reads
  its count under the lock, releases it, and waits until the count
  differs. No wake-up is lost, and a waiting thread uses no processor.
- **`sync-exempt`** (CHG-0072): a lock-derived path and a shared path
  that is not lock-derived never clash, whatever the shared path's
  type. The interior of a mutex is reachable only through a lock, so a
  shared path to a struct around it can reach nothing a lock path does.

## Rejected alternatives

- **2:** a built-in type is a second set of rules in both tools for
  what `std` can say in CobaltC over `mutex`.
- **3:** a condition variable is the hard part of every channel
  written by hand; a program would get it wrong.
- **5:** a fast producer grows the queue without limit; a bound is the
  back-pressure a pipeline needs.
- **6:** a rendezvous is a different protocol from a queue; one
  constructor and one meaning keep it simple.
- **8:** counting senders means a sender handle type and a second
  resource per thread, for what one `close` call says.
- **10:** deferred until a program needs to poll.

## Semantic rationale

A channel is a `mutex` around a ring of `Option<T>` slots; each
operation is a locked step over it, and waiting is the absence of a
step, as in `[Lock]`. A value moves into the channel on `send` and out
on `recv`, so it has one owner throughout.

## Usability

    fn worker(ref<Channel<Job>, shared> jobs, ref<Channel<u64>, shared> results)
    {
        while (true)
        {
            match (Channel::recv(jobs))
            {
                Some(j) : Channel::send(results, run(j)),
                None : break,
            }
        }
    }

The main thread spawns the workers, sends the jobs, closes `jobs`, and
joins them.

## Implementation-feasibility

`std` source (`Channel` and its functions) over `mutex`, `lock` and one
primitive, `event_op`. In `coby` the counts sit beside the lock table
the waiting threads poll; in `cbrt` they use the runtime's existing
wait/signal machinery (`wait_until`, `signal`), as `lock` and `join` do.

## Compatibility impact

Extension. A program's own `Channel` takes precedence (D-0024).
`sync-exempt`'s widening accepts programs that were rejected; none
that was accepted changes.

## Prior-art status

- **Go:** `make(chan T, n)`, `ch <- v`, `v, ok := <-ch`, `close(ch)`;
  sending on a closed channel panics.
- **Rust:** `std::sync::mpsc::sync_channel(n)`; `send` gives the value
  back in its error; the channel closes when the senders are dropped.
- **Java:** `ArrayBlockingQueue`.

## Revisit conditions

- `try_send`/`try_recv`, a timeout, or selecting over several channels.
- An unbounded channel, if a program needs one.
- Detecting more deadlocks (every thread waiting).
