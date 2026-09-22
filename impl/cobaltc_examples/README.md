# CobaltC examples

Thirteen small programs, each verified with `coby`. All but the last
are single files; `13_modules.cb` declares `13_geometry.cb` as the body
of a module (`spec/17` §5), so that pair is one program. Run one with:

    coby 03_primes.cb

Every program writes its output with `printf` (`spec/21` §2f), which
formats text and numbers and is checked when the program is compiled.

| File | What it shows | Output |
|---|---|---|
| `01_hello.cb` | A `str` literal and `printf` -- no `unsafe`, no pointer, no byte in the program | `Hello, CobaltC!` |
| `02_fizzbuzz.cb` | `while`, `if`/`else if`, `%`, `printf` for words and numbers | FizzBuzz 1..30 |
| `03_primes.cb` | `Vec<bool>` sieve, writing through `Vec::index_exclusive` | primes below 100, then `25` |
| `04_ownership.cb` | `resource struct` with a destructor, moves vs borrows, reverse-order destruction | `c b d a`, one per line |
| `05_shapes.cb` | Enums with payloads, exhaustive `match`, `f64` printed with `%.2f` | four areas with two decimals |
| `06_errors.cb` | `Result`, the `?` operator, `map_err` between error types | `ok 42`, `err 2`, `err 1` |
| `07_generic_stack.cb` | A generic `Stack<T>` over `Vec<T>`, `Option` from `pop`, an RPN evaluator | `1`, `56`, `2` |
| `08_closures.cb` | Borrow and `move` captures, named functions as `fn(..) : T` values, map and fold | transformed vectors, `430`, `4` |
| `09_threads.cb` | `spawn`/`join` with return values, a `mutex` counter, handles swept at block exit | `499500`, `500` |
| `10_shared_rc.cb` | `Rc` shared ownership, `Rc::clone`, read-only access through `Rc::get` | `60`, `50` |
| `11_checked_arithmetic.cb` | `checked_*`, `wrapping_*`, `saturating_*`, `narrow_wrapping` | eight lines of results |
| `12_collatz.cb` | Memoised search with `break`, `if` as an expression, reborrowing `&*r` | `6171`, `261` |
| `13_modules.cb` | A program spanning two files: `module geo "./13_geometry.cb";` makes that file's items the module body; `import` across the boundary | `12`, `14` |

Two of the files have a commented-out line that the static checker
would reject (`04_ownership.cb`, `08_closures.cb`) and one has a line
that would fault at run time (`11_checked_arithmetic.cb`). Uncomment
them to see the diagnostics, which name the rule, the line, and a
suggested repair.
