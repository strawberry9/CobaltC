# Guide generator

Regenerates `../index.html`, the human-readable CobaltC language guide.

    cd ~/CobaltC/htmlguide/generator
    python3 gen.py --check    # run every complete example through ~/CobaltC/impl/target/release/coby
                              # and cobc --run beside it, once per C compiler (gcc and clang); all must
                              # match the expected verdict and output (set COBALTC=/path/to/coby and
                              # COBC=/path/to/cobc to point elsewhere, COBC_CCS="gcc" for one compiler)
    python3 gen.py            # write ../index.html

Files:

- `gen.py` — page shell, CSS, syntax highlighting, HTML escaping, and the example checker.
- `content_intro.py` — introduction and the tour program.
- `content_a.py` — sections §06–§11.  `content_b.py` — §12–§16.  `content_c.py` — §17–§22 and the appendix.

Each `code(...)` call with `expect="ok"` or `expect="diag.xxx"` is verified by `--check`
(the extracted programs land in `./check/`, which is disposable). `interp=` marks a place
where the reference interpreter's verdict differs from the specification's; none is needed
at present. `files={"name.cb": src}` gives a multi-file program its companion files (the
program is then saved as `main.cb` in its own subdirectory of `./check/`, so a relative
`module m "./name.cb";` resolves). `args=[…]` passes arguments to the program under both
tools, and `status=N` expects an `"ok"` example to exit with status `N` (`fn main() : u8`).
Requires Python 3 only.
