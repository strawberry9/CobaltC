# CHG-0466 — the gate's Wine unit leaves out the two cases Wine cannot run (implementation only)

Status: implemented (2026-11-17); Update 196
Affects: `impl/tools/gate.py` (`wine`), `impl/conformance/run.py` (`--skip`), `impl/conformance/README.md`,
`README.md`

## Why

Two cases start 300 threads (`19-concurrency/heaps_meet_ok.cb`, `many_threads_owning_frames_ok.cb`). Wine 6
makes each thread's whole reserved stack resident, 64 MB a thread, 19 GB in all (D-0255); the machine has 16.
The `wine` unit ran them all the same and did not count their failure: each grew until the kernel's
out-of-memory killer ended it, at 12 to 14 GB, twice in every run of the unit (the kernel's log has every one).
On 2026-11-17 a run did not survive it: with the machine's memory all but gone, the session the gate ran in
stopped the run, the release profile with three units left (`wine`, `programs`, `external`).

No rule changes; the two cases run as before on Linux and on GitHub's Windows runner.

## What changes

- `impl/conformance/run.py` takes `--skip TEXT`, as often as wanted: the cases whose path contains the text
  are left out and counted as skipped.
- The `wine` unit leaves the two cases out, and is green only when no case fails (it had passed over a failure
  of either of the two).
- The unit runs under a memory cap of 10 GB where systemd offers the user one (`systemd-run --user --scope`),
  and says so when it does not. The largest case that does run under Wine, `thread_failures_many_ok.cb` with
  200 threads, takes 5.7 GB; a case that came to take the machine's memory would be ended at the cap.

Tried and not kept: a cap alone, 6 GB over the whole unit. The two cases were ended at the cap in eight
seconds, but so was `thread_failures_many_ok.cb`, running beside one of them.

## Checked

- `19-concurrency` under Wine as the unit now runs it (three at a time, the cap, the two left out): 144 of
  144 pass, 2 skipped; the least memory the machine had available through the run was 11.5 GB, and the
  kernel's log has no kill.
- The cap: a program touching 3 GB under a 1 GB cap is ended at 1 GB; one of the two cases under a 6 GB cap is
  ended at 6 GB, in 8 seconds.
