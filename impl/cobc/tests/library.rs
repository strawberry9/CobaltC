// D-0257: library mode. `cobc --lib` builds a static library of a
// program's `extern fn` definitions, with a header; a C program links it
// (and the runtime beside it) and calls in: plain values, structs by value,
// buffers and strings through `Vec::from_raw`, `copy_to_raw` and
// `String::from_c`, from one thread and from four C threads at once; a
// fault inside a `_try` call is contained and reported, inside the plain
// form it ends the process. Under each C compiler.

mod oracle;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LIB: &str = r#"import std;

struct Point
{
    f64 x;
    f64 y;
}

struct Stats
{
    u64 count;
    u64 sum;
    Point centre;
}

fn sum_bytes(slice<u8, shared> data) : u64
{
    u64 s = 0;
    foreach (b in data)
    {
        s += widen<u64>(*b);
    }
    s
}

extern fn byte_sum(rawptr<u8> data, usize len) : u64
{
    Vec<u8> bytes = unsafe { Vec::from_raw(data, len) };
    sum_bytes(&bytes[0 .. $])
}

extern fn midpoint(Point a, Point b) : Point
{
    Point { .x = (a.x + b.x) / 2.0, .y = (a.y + b.y) / 2.0 }
}

extern fn stats(rawptr<u64> values, usize n) : Stats
{
    Vec<u64> v = unsafe { Vec::from_raw(values, n) };
    u64 sum = 0;
    foreach (x in &v)
    {
        sum += *x;
    }
    Stats { .count = widen<u64>(n), .sum = sum, .centre = Point { .x = to_float<f64>(sum) / to_float<f64>(n), .y = 0.5 } }
}

extern fn utf8_chars(rawptr<u8> text) : i64
{
    match (unsafe { String::from_c(text) })
    {
        Ok(s) : reinterpret<i64>(widen<u64>(narrow<u32>(Vec::len(&String::chars(&s))))),
        Err(_) : -1,
    }
}

extern fn fill_squares(rawptr<u32> out, usize n) : usize
{
    Vec<u32> v = Vec::new();
    for (u32 i = 0; i < narrow<u32>(n); i += 1)
    {
        Vec::push(&mut v, i * i);
    }
    unsafe
    {
        copy_to_raw(&v[0 .. $], out);
    }
    n
}

extern fn divide(i64 a, i64 b) : i64
{
    a / b
}

extern fn say(u32 n)
{
    printf("said %v\n", n);
}
"#;

const MAIN_C: &str = r#"#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <pthread.h>
#include "geo.h"

static void *worker(void *arg)
{
    long id = (long)arg;
    uint8_t buf[256];
    for (int i = 0; i < 256; i++) buf[i] = (uint8_t)(i + id);
    uint64_t acc = 0;
    for (int r = 0; r < 100; r++) acc += byte_sum(buf, 256);
    return (void *)(uintptr_t)acc;
}

int main(void)
{
    /* The library writes its output as it returns; C's stdio keeps its
       own buffer, so it is switched off here to keep the two in order. */
    setvbuf(stdout, NULL, _IONBF, 0);
    const char *text = "abc";
    printf("sum %llu\n", (unsigned long long)byte_sum((uint8_t *)text, strlen(text)));
    struct Point a = { 1.0, 2.0 }, b = { 3.0, 6.0 };
    struct Point m = midpoint(a, b);
    printf("midpoint %.1f %.1f\n", m.x, m.y);
    uint64_t vals[5] = { 1, 2, 3, 4, 5 };
    struct Stats s = stats(vals, 5);
    printf("stats %llu %llu %.1f %.1f\n", (unsigned long long)s.count, (unsigned long long)s.sum, s.centre.x, s.centre.y);
    printf("chars %lld\n", (long long)utf8_chars((uint8_t *)"h\xc3\xa9llo \xe2\x82\xac"));
    uint32_t sq[4];
    fill_squares(sq, 4);
    printf("squares %u %u %u %u\n", sq[0], sq[1], sq[2], sq[3]);
    say(7);
    int64_t q = 99;
    int rc = divide_try(7, 2, &q);
    printf("try %d %lld\n", rc, (long long)q);
    q = 99;
    rc = divide_try(7, 0, &q);
    char msg[64];
    int n = cobalt_last_fault(msg, sizeof msg);
    printf("try %d %lld %d %.24s\n", rc, (long long)q, n > 60, msg);
    printf("again %llu\n", (unsigned long long)byte_sum((uint8_t *)text, strlen(text)));
    pthread_t t[4];
    for (long i = 0; i < 4; i++) pthread_create(&t[i], NULL, worker, (void *)i);
    uint64_t total = 0;
    for (int i = 0; i < 4; i++) { void *r; pthread_join(t[i], &r); total += (uint64_t)(uintptr_t)r; }
    printf("threads %llu\n", (unsigned long long)total);
    cobalt_shutdown();
    printf("done\n");
    return 0;
}
"#;

const ABORT_C: &str = r#"#include <stdio.h>
#include "geo.h"
int main(void)
{
    printf("before\n");
    fflush(stdout);
    long long r = divide(1, 0);
    printf("not reached %lld\n", r);
    return 0;
}
"#;

fn scratch(cc: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cobc-library-{}-{}-{}", std::process::id(), cc, name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// The C link line the header states, as a C compiler takes it.
fn link_line(dir: &Path) -> Vec<String> {
    let h = fs::read_to_string(dir.join("geo.h")).expect("the header");
    let line = h.lines().find(|l| l.contains("Link,")).expect("the link line");
    let after = line.split("(-L): ").nth(1).expect("the libraries").trim();
    after.split_whitespace().map(str::to_string).collect()
}

#[test]
fn a_c_program_links_and_calls_the_library() {
    let results = oracle::per_compiler(|cc| {
        let dir = scratch(cc, "geo");
        fs::write(dir.join("geo.cb"), LIB).unwrap();
        fs::write(dir.join("main.c"), MAIN_C).unwrap();
        fs::write(dir.join("abort.c"), ABORT_C).unwrap();
        let out = oracle::cobc(cc).args(["--lib", "-o"]).arg(dir.join("libgeo.a")).arg(dir.join("geo.cb")).output().expect("run cobc");
        let built = (out.status.code(), String::from_utf8_lossy(&out.stderr).into_owned());
        if built.0 != Some(0) {
            let _ = fs::remove_dir_all(&dir);
            return (built, None, None);
        }
        let libs = link_line(&dir);
        let header = fs::read_to_string(dir.join("geo.h")).unwrap();
        let mut c = Command::new(cc);
        c.args(["-O2", "-w", "-o", "main.bin", "main.c", "-L."]).args(&libs).current_dir(&dir);
        let cst = c.output().expect("run the C compiler");
        assert!(cst.status.success(), "[{}] C: {}", cc, String::from_utf8_lossy(&cst.stderr));
        let mut prog = Command::new(dir.join("main.bin"));
        oracle::limited(&mut prog);
        let run = prog.current_dir(&dir).output().expect("run the program");
        let mut c2 = Command::new(cc);
        c2.args(["-O2", "-w", "-o", "abort.bin", "abort.c", "-L."]).args(&libs).current_dir(&dir);
        assert!(c2.output().expect("run the C compiler").status.success(), "[{}] abort.c", cc);
        let mut prog2 = Command::new(dir.join("abort.bin"));
        oracle::limited(&mut prog2);
        let run2 = prog2.current_dir(&dir).output().expect("run the program");
        let _ = fs::remove_dir_all(&dir);
        (built, Some((header, run.status.code(), String::from_utf8_lossy(&run.stdout).into_owned())), Some((run2.status.code(), String::from_utf8_lossy(&run2.stdout).into_owned(), String::from_utf8_lossy(&run2.stderr).into_owned())))
    });
    for (cc, (built, ran, aborted)) in results {
        assert_eq!(built.0, Some(0), "[{}] cobc --lib: {}", cc, built.1);
        let (header, code, stdout) = ran.unwrap();
        assert!(header.contains("struct Point {") && header.contains("struct Stats {"), "[{}] header: {}", cc, header);
        assert!(header.contains("_Static_assert(sizeof(struct Stats) == 32"), "[{}] header: {}", cc, header);
        assert!(header.contains("uint64_t byte_sum(uint8_t *data, uint64_t len);") && header.contains("int divide_try(int64_t a, int64_t b, int64_t *result);"), "[{}] header: {}", cc, header);
        // An entry's own `out` does not clash with the `_try` form's result parameter.
        assert!(header.contains("int fill_squares_try(uint32_t *out, uint64_t n, uint64_t *result);"), "[{}] header: {}", cc, header);
        // Four threads, 100 sums each of the 256 bytes `i + id` wrapped in
        // u8 (a rotation of 0..255, sum 32640): 4 × 100 × 32640.
        let expect = "sum 294\nmidpoint 2.0 4.0\nstats 5 15 3.0 0.5\nchars 7\nsquares 0 1 4 9\nsaid 7\ntry 0 3\ntry 1 99 1 fault[diag.div-by-zero]:\nagain 294\nthreads 13056000\ndone\n";
        assert_eq!((code, stdout.as_str()), (Some(0), expect), "[{}]", cc);
        let (code2, out2, err2) = aborted.unwrap();
        assert_eq!((code2, out2.as_str()), (Some(1), "before\n"), "[{}] stderr: {}", cc, err2);
        assert!(err2.starts_with("fault[diag.div-by-zero]"), "[{}] stderr: {}", cc, err2);
    }
}

// A library is not a program, and a program is not a library: each is
// refused as the other, with the way out named.
#[test]
fn a_library_and_a_program_are_told_apart() {
    let dir = scratch("gcc", "kinds");
    fs::write(dir.join("lib.cb"), "import std;\n\nextern fn one() : u32\n{\n    1\n}\n").unwrap();
    fs::write(dir.join("prog.cb"), "import std;\n\nfn main()\n{\n    printf(\"1\\n\");\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).arg("lib.cb").current_dir(&dir).output().expect("run cobc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{}", err);
    assert!(err.contains("is a library") && err.contains("cobc --lib -o liblib.a lib.cb"), "{}", err);
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--lib", "prog.cb"]).current_dir(&dir).output().expect("run cobc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{}", err);
    assert!(err.contains("has no `main`"), "{}", err);
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--lib", "--run", "lib.cb"]).current_dir(&dir).output().expect("run cobc");
    assert_eq!(out.status.code(), Some(2));
    // The default output name: `lib<stem>.a` beside the source, and its
    // header and the runtime with it.
    let out = Command::new(env!("CARGO_BIN_EXE_cobc")).args(["--lib", "lib.cb"]).current_dir(&dir).output().expect("run cobc");
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.join("liblib.a").is_file() && dir.join("lib.h").is_file() && dir.join("libcbrt.a").is_file());
    let _ = fs::remove_dir_all(&dir);
}
