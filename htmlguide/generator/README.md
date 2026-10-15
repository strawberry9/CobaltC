# Guide generator

Regenerates `../index.html`, the human-readable CobaltC language guide, and
a copy, `../Appendix_06.html`, for publishing elsewhere (replaced every time;
the same page, with its images loaded from the page's own directory). The preface at the top of the page is `preface.html`.

    cd ~/CobaltC/htmlguide/generator
    python3 gen.py --check    # run every complete example through ~/CobaltC/impl/target/release/coby
                              # and cobc --run beside it, once per C compiler (gcc and clang); all must
                              # match the expected verdict and output (set COBALTC=/path/to/coby and
                              # COBC=/path/to/cobc to point elsewhere, COBC_CCS="gcc" for one compiler)
    python3 gen.py            # write ../index.html and ../Appendix_06.html

Files:

- `gen.py` — page shell, CSS, syntax highlighting, HTML escaping, and the example checker.
- `content_intro.py` — introduction and the tour program.
- `content_cprog.py` — the section for C programmers (what carries over, the same words with another
  meaning, the habits the compiler stops, error handling at length).
- `content_rust.py` — the section comparing CobaltC with Rust: what they share, where they differ and the
  price and payoff of each choice, without a verdict.
- `fonts/` — the code font, Cascadia Mono (Google Fonts' latin subset, a variable font) and its
  licence, `OFL.txt` (SIL Open Font License 1.1). `gen.py` embeds it in the page as base64, so the
  guide is one self-contained file that loads nothing from elsewhere.
- `content_a.py` — sections §06–§11.  `content_b.py` — §12–§16.  `content_c.py` — §17–§22 and the appendix.
- `std_inventory.py` — §21's listing of every std module and item, read from `impl/std` and the tools.
- `std_reference.py` — the std reference (section R): every exported std item's declaration,
  `///` documentation and examples, read by `impl/tools/cobdoc.py`. An example is a documented
  `unittest` block after its item; the page shows its body and `--check` runs it as `fn main()` of a
  program importing `std` and the item's module. Edit the documentation in `impl/std`, never here.

Each `code(...)` call with `expect="ok"` or `expect="diag.xxx"` is verified by `--check`
(the extracted programs land in `./check/`, which is disposable). `interp=` marks a place
where the reference interpreter's verdict differs from the specification's; none is needed
at present. `files={"name.cb": src}` gives a multi-file program its companion files (the
program is then saved as `main.cb` in its own subdirectory of `./check/`, so a relative
`module m "./name.cb";` resolves). `args=[…]` passes arguments to the program under both
tools, and `status=N` expects an `"ok"` example to exit with status `N` (`fn main() : u8`).
Requires Python 3 only.
