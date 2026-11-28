# D-0311 — What ten sessions of ordinary programs found, and what was decided about it

Status: ACCEPTED (2026-11-06, under the owner's delegation for this work: "I leave it to you to solve all the
issues autonomously. Do not stop to ask me," after reading the report of the findings); implemented by `CHG-0412`
Kind: Decision (`spec/02-schema.md`)
Governed by: `CobaltC_Master_Instructions.md`
Depends on: D-0042 and D-0287 (`foreach` and its expansion), D-0073 and D-0135 (temporaries borrowed, views of
them), D-0149 and D-0162 (a text literal where a `String` is declared), D-0213 (`[Thread-Lend]`), CHG-0081 and
CHG-0334 (disambiguation (4); a type is no value), D-0284 and D-0285 (decided programs), D-0308
Affects: `spec/14` §6, `spec/17` `[Resolve-Unqualified]`, `spec/19` `[Spawn]`, `spec/21` (three functions),
`spec/22` §2 disambiguation (4); `spec/conformance.md` (nine rows); `coby`, `cobc`, `cbrt`, `cbc`; the book and the
guide

## 1. Where the findings came from

Another agent wrote about a hundred and thirty programs in CobaltC, in ten sessions on another machine (games,
algorithms, numerics, concurrency, systems tools, interpreters, simulations, protocols), and kept a file of what
went wrong in each. The owner asked for the files to be read together, later ones correcting earlier ones, and for
a report of what needs addressing; then for everything in it to be addressed. The report and its reproductions are
kept outside the repository. This record says what was wrong in the language's tools and what was decided where a
rule or the standard library had to change.

## 2. Defects in the implementations (no rule changed)

1. **A compiled loop read freed memory.** `foreach (t in StringView::split(StringView::trim(&f()), " "))`: the
   `String` `f()` returns ends with the loop's header, while every part the loop takes is a view of it.
   `spec/14` §6 already refutes a binding whose facts name a temporary and that is live after its statement
   (D-0135), but every node of the expansion has the `foreach`'s position, so the pass's liveness test could not
   see the loop use its hidden holder after the statement that makes it; the body was decided, the program kept
   no record, and `cobc`'s build read the freed bytes (ASan: heap-use-after-free). The hidden holder is now always
   live after its statement, and the loop is rejected with a message that names the repair. `spec/14` says so.
2. **The same file gave different answers on different runs.** The static pass keeps facts about expressions in
   tables keyed by the expression's address; a `foreach` expansion was checked and then freed, so a later node at
   a reused address took its facts. The static check of one file was rejected in 7 to 14 runs of 20; `cobc`'s
   decided lowering stopped with "a StringView operation of an unexpected shape" in about a third of the builds of
   another, and fell back to the checked lowering. Every expression the pass makes and checks is now kept for the
   tables' lifetime (`Items::each_expansions`, `kept_exprs`), the lowering lowers the pass's own expansion, every
   node the lowering synthesizes is kept for the whole lowering (`Gen::forever`), and copying facts to a copy is
   exact. Builds are now byte-identical; a test builds a program several times and compares the C.
3. **`cobc --explore` did not explore a decided program's locks.** A release only makes a switch pending, taken
   at the thread's next entry into the runtime; a decided program's lock (`cb_lock_pathless`) makes none, so every
   schedule ran the book's two bumps one after the other. The pathless lock is now such a point under a schedule.
4. **A type error was accepted.** `Vec::len(&Result::unwrap(o))` with `o` an `Option`: an inner generic call under
   an expected type not yet fixed deferred every open type parameter, and nothing asked again; `coby` then ran the
   ill-typed program. Only a parameter the callee's result mentions is deferred now: no expected type can fix
   another.
5. **Internal errors in `cobc`** on programs the check accepts: `f(0) == Err(Bad)` and `o == None` where the enum
   is a resource or the variant is on the left (the other operand's type now types the variant); `row.counts[i]`
   through a `foreach` element, and `a[0].path` in a `sort_by` comparator (a shared borrow of a place reached with
   no path, every access to it proven, is its address).
6. **A false rejection**, `diag.reference-escapes-scope` on a nested `foreach` inside a `foreach` over a call's
   result, was the stale facts of item 2 met deterministically.
7. **`Vec::len(&f(x))` evaluated `f(x)` twice** in `cobc` (found while checking the findings): a double side
   effect, and `diag.stale-binding` when the call consumed a local.
8. **An atomic operation kept its function's frame and statement scopes** (a global flag the scope analysis did
   not know read nothing), 195 ns a store and a load; now 10 ns.
9. **Tools:** `coby --explore` runs its schedules in parallel and no longer waits for an open input to end before
   the first; `coby --unittest` counts a test that stops on a limit of the tool (`unsupported: …`) apart, not as a
   failure; each test's label shows while it runs.

## 3. Decisions

1. **A loop over views of its header's temporary is rejected before the run** (item 2.1), not left to the run as
   the session suggested might also do: the rule already refutes it, and the program can never run correctly.
2. **In value position a module is passed over, as a type is** (`spec/17` `[Resolve-Unqualified]`, extending
   CHG-0334). A module is no value, so beside `module check "check.cb"; import check;`, `check(5)` is the module's
   exported `check` by clause (2b); it was `diag.unbound-name`, whose repair ("add an `import`") was wrong. The
   alternative, a better message only, was rejected: nothing else `check(5)` could mean.
3. **`Name<…> x =` (or `;`) is a declaration** (`spec/22` §2 disambiguation (4)), as `T x = …;` is since CHG-0081:
   by (1) the list is type arguments, and a name after them begins no expression. A file read alone (`cobfmt`,
   `--check-syntax`) no longer fails on a local whose generic type another module declares.
4. **`spawn`'s other arguments are typed as a call of the function** (`spec/19` `[Spawn]`): a text literal where a
   `String` is declared is that `String` (D-0149), as the implementations' own message already said.
5. **Three additive `std` functions:** `StringView::rfind`, `StringView::parse_radix<T: integer>` and
   `Vec::eq<T: eq>` (`spec/21`), each written by hand in many of the programs.
6. **Declined, with what to write instead:**
   - `assert(a == b)` showing both values: the message would evaluate the operands again, wrong for a call or a
     resource; binding them first is a larger design. The failing line is shown.
   - A lookup in a `HashMap<String, V>` by a `StringView`: D-0308 retired the `_str` lookups for literals; a view
     key needs a `String` built first, or a design for borrowed keys, not taken up here.
   - Pushing a non-ASCII byte into a `String`: a `String` is UTF-8; build the bytes in a `Vec<u8>` and use
     `String::from_utf8`.
   - A relative import inside a library (`import self::book;`): the book now says a library fixes the name it is
     mounted under; a form for it is left for a later decision.
   - File modes, owners, links and `truncate`: platform surface on every target; the guide says what is missing
     and how a program does without.
   - Compiling one test alone: `--unittest-filter` already runs one; the build is the program's.
7. **Already in the language**, the findings mistaken: a rest pattern in destructuring (`Name { f, .. } = x;`),
   octal literals (`0o755`), and typed integer literals (`2: usize`); the book's and the guide's text was made
   clearer where it was easy to miss.

## 4. Diagnostics and documentation

Messages now name the fix for `!` on an integer (`~`), `auto` as a parameter's type, `as<T>(x)` (the conversion
functions), `: T` after a range of variables, a `str` where a `String` field is declared, a `str` sliced, and a
place lent to a thread a `Vec` of handles holds. The book gained a `map_err` example with `std`'s `ParseError`
(ch09), the types of a range and of an array literal's elements (ch05), and std's short names, a library's mounted
name, and the test runner's filter, `UNSUPPORTED` and progress (ch12); the guide, `%v`'s flags, what `FileInfo`
lacks, `Rng`'s algorithm, and when atomics and freshly built vectors cost.
