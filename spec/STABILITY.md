# CobaltC Stability Policy

Status: non-normative artifact (like `IMPLEMENTATION-NOTES.md`: it carries
no rule, invariant, type or state entity). It says what an edition promises
about the editions that follow it. Decided by D-0220.
Governed by: `CobaltC_Master_Instructions.md` §21 (change management)

## 1. Two parts, each with a mark

The specification has two parts, and each edition marks each one:

| Part | What it is |
|---|---|
| **core** | the language: `00`–`20` and `22`, the diagnostics and feature registries, and the conformance rows and cases of those chapters |
| **std** | the standard library: `21`, and the conformance rows and cases of `21` |

A part's mark is one of three words, in the order a part moves through them:

- **moving**: any change may come through the process (a `D-nnnn` decision, a
  `CHG-nnnn` record, the spec text, both implementations, conformance cases,
  the guide), including a change that rejects a program an earlier edition
  accepted, or changes what one does.
- **candidate**: the part is offered as stable. For the *candidate period*,
  thirty days from the edition that first marks it candidate, the part
  takes only clarifications and soundness repairs (§2). Any other change
  withdraws the candidacy: the edition that makes it marks the part moving
  again, and a later edition may mark it candidate afresh.
- **stable**: the candidate period ended without a withdrawal, and an
  edition marked the part stable. From then on a change to the part is one
  of the four kinds of §2, and a breaking one follows §3.

The marks appear at the end of each row of the edition register
(`README.md`), after the evidence tier: `Stability: core moving, std moving`.
A row without them (every row before D-0220) is read as both parts moving.

## 2. Kinds of change

Every change to a part is one of these, and its edition row says which (the
words the register already uses):

| Kind | What it may do | Where it may land |
|---|---|---|
| **Clarification** | change text so that it says what the implementations and the conformance cases already do, or what an earlier text plainly meant; no accepted program rejected, no outcome changed | always |
| **Soundness repair** | reject, or fault, a program whose run could break an invariant of `03` or the safe-program guarantee (`term.safe-program`); every program it newly rejects is one that could reach unsafe execution or a false trusted claim, and its decision record shows how | always, with a decision record and a conformance case per shape it closes |
| **Additive** | accept programs no earlier edition accepted, or add `std` items, without changing what any accepted program does | moving; stable (a new edition); never during a candidate period of the part it adds to |
| **Breaking (source)** | anything else: rejects an accepted program for another reason, or changes what one does | moving; stable only by §3 |

A soundness repair is the one way a stable part rejects what it accepted.
It is not a loophole: the decision record names the invariant, the
reachable unsafe step, and the programs it rejects, and an independent
reviewer can check each claim. A change that rejects safe programs along
with unsafe ones is breaking, and goes through §3, unless no narrower rule
closes the hole; then the record says so.

## 3. Breaking a stable part

A breaking change to a stable part needs:

1. a decision record that states the change, the programs it affects, and
   why an additive form cannot do;
2. a **deprecation window**: the edition that introduces the new form keeps
   accepting the old one with its old meaning, and says in its row which form
   is deprecated; the old form is removed no earlier than the second stable
   edition after that, and no earlier than ninety days after it;
3. while the window is open, the old form is `[[deprecated("…")]]` (D-0303,
   `spec/17` `rule.module.retired`): both implementations reject a use of it
   unless `--allow-deprecated` is given, and then accept it with its old
   meaning and name each use in a warning; the guide shows the new form only,
   with one sentence on the old.

A deprecated form keeps its meaning exactly under the allowance. The default
is the error: a second spelling of one thing does not survive the window by
inertia. When the window closes the form becomes a tombstone
(`[[removed("…")]]`), which no flag reopens.

## 3a. A name's lifecycle

| Step | Written as | A use is | When it may advance |
|---|---|---|---|
| live | the declaration | accepted | — |
| deprecated | `[[deprecated("m")]]` on the declaration, which keeps its body and its meaning | `diag.deprecated-use`; a warning under `--allow-deprecated` | by a decision record; the edition row names the name |
| removed | the tombstone, `[[removed("m")]]` and the item's head | `diag.removed-use`; no flag accepts it | in a stable part, when §3's window closes; in a moving part, not before the next published edition after the one that deprecated it, and never in the same Update |
| deleted | nothing | `diag.unbound-name` | optional: a tombstone is one line and may stand indefinitely. `std` keeps its tombstones at the end of the module's file under `// Retired (D-nnnn)`, and `spec/21` lists them in a "Retired" table at the end of the section they belonged to |

## 4. When a part becomes a candidate

An edition marks a part candidate only when all of these hold for it:

- the gate's release profile (`impl/gate.sh release`, D-0307) is green on
  the edition, with its verdicts named in the manifest;
- the model sample (`impl/model`, D-0219) agrees with both implementations;
- no soundness finding of priority 1 is open against the part, from any
  review, campaign or model run;
- the most recent independent review of the part's chapters found no
  priority-1 defect, or each one it found is repaired and the repair checked
  by the model or a conformance case;
- for **std**: its module map has been declared fixed by a decision, every exported item has a
  conformance case, and every parser of a security-relevant module passed a
  full fuzzing budget.

## 5. What this means for a reader

- **A program** written against a stable part keeps compiling, and keeps
  doing what it did, in every later edition, except where a soundness repair
  rejects it — and then its decision record explains why it could fail
  unsafely — or, during a deprecation window, builds only with
  `--allow-deprecated`, or after a deprecation window has run its course.
- **An implementation** claims conformance to an edition (`NAMING-POLICY.md`);
  the claim inherits the edition's marks.
- **The guide** says, near its start, which marks the current edition has.

## 6. The current marks

As of the edition that adopted this policy, both parts are **moving**.

## Change Log

- 1.1.1 — D-0307 (`CHG-0406`): §4's first condition is the gate's release profile.
- 1.1.0 — D-0303 (`CHG-0401`): §3's window is `[[deprecated]]`, an error unless `--allow-deprecated`; §3a, a
  name's lifecycle; §5's first bullet.
- 1.0.0 — D-0220 (`CHG-0259`): the policy.
