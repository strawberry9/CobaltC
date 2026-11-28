# CHG-0461 — The gate's units use the three processes they are allowed

Status: implemented (2026-11-16); Update 195
Affects: `impl/tools/gate.py`, `impl/build.sh`, `htmlguide/generator/gen.py`, `showcase/run_all.sh`

## Why

The owner, 2026-11-16, on the gate's two hours: "stop and optimise the gate first". A unit may run three
processes at a time (D-0307); several ran one. From the last full run's logs (`20261115_215656`, 2 h 08 min):
`bootstrap` built from the seed with GCC and then with Clang (11 and 13 minutes); `corpus` ran the 683 cases
carried over from `tests/conformance.rs` in one process (6½ minutes); `guide` and `showcase` ran one program at a
time (11 minutes each); `model` ran five readings that share nothing one after another (9 minutes); `build`
built the compiler twice at full optimisation after a change of the runtime, then five programs and the seed's
check in sequence.

## What changes

- **`bootstrap`**: the two C compilers' builds side by side.
- **`corpus`**, **`tests`**, **`all-checks`**: the cases carried over from `tests/conformance.rs`, the `cbtest`
  cases and the suite with every check in three shards, as the suite and the rows were.
- **`guide`** and **`showcase`**: three programs at a time (`JOBS=3`; `gen.py --check` and `run_all.sh` take
  `JOBS`, default one), reported in their order. A guide example runs in a directory of its own under `check/`,
  so two examples that write a file of one name do not meet.
- **`model`**: the comparison three programs at a time; the five readings in three chains.
- **`build`**: after a change of the runtime the first stage's compiler is built at `-O 1` (its C compiles in
  under half the time; it runs once, writing the same C a fifth slower); the five programs after `cobc` two at a
  time (`impl/build.sh`'s `JOBS`), beside the check that the committed seed is current.
- **`wine`**: three jobs.

What a unit checks is unchanged: the same commands over the same inputs, more of them at a time. Not changed:
the units still run one after another, and a unit still has three processes at most, so one of this machine's
four cores stays free; running a single-process unit (`fixpoint`, about two hours) beside the others would
take that core, which is the owner's to decide.

## Checked

- Each changed piece by itself, the counts as before: the guide's 426 examples, none failing, in 4.2 minutes
  (11 before); the 91 showcases in 5.2 (11); the 143 `cbtest` cases in three shards in 2.1 (3.5); the five
  programs in 3.1 (5.0).
- The compiler building itself: 135 s to write its C; the C compiled in about 275 s at `-O 2`, 123 s at `-O 1`,
  75 s at `-O 0`; a compiler built at `-O 1` writes that C in 163 s, one built at `-O 0` in over five minutes.
- **The gate profile whole**, on Update 195's commit (`ae6ae00`, run `20261116_095541`): 15 units green in 87
  minutes, where the same units took 128 before: build 9 (18), tests 2 (3), corpus 19 (23), guide 4 (11),
  showcase 5 (11), model 6 (11), bootstrap 14 (24), lowering 19 (18).
