# Functions and control flow

CobaltC's control flow is C's, trimmed and tidied: `if`, `while`, `for`, and a
`foreach` for walking collections; no `switch`, no `goto`, no `do`. What is
new is that blocks, `if` and `match` are *expressions* with values, which
changes how you write a lot of ordinary code, and that the end of a block is
a precise moment at which things happen.

## Functions

```
import std;

// Parameters are type-first; the return type follows a colon.
fn area(i32 width, i32 height) : i32
{
    width * height                    // the last expression is the result
}

// No return type: the function returns nothing. `return;` leaves early.
fn report(i32 w, i32 h)
{
    if (w <= 0 || h <= 0)
    {
        printf("no area\n");
        return;
    }
    printf("area %v\n", area(w, h));
}

fn factorial(u64 n) : u64
{
    if (n <= 1)
    {
        1
    }
    else
    {
        n * factorial(n - 1)
    }
}

fn main()
{
    report(3, 4);
    report(3, 0);
    printf("%v\n", factorial(20));
}
```

```
area 12
no area
2432902008176640000
```

A function's body is a block, and a block's value is its last expression,
written without a semicolon. That is how `area` returns a value with no
`return`, and how `factorial` returns whichever branch of its `if` runs.
`return e` still exists, and is the way to leave early; a function whose
natural shape is "check something, then do the work" reads best with an early
`return` for the check and a trailing expression for the work.

Functions may be declared in any order and may call themselves. They cannot
be nested: a function inside a function is written as a closure, which
chapter 11 covers. A function that belongs with a type is written
`fn Type::name(...)` and called as `Type::name(...)`; chapter 8 shows that.

## Blocks and `if` are expressions

```
import std;

fn main()
{
    i32 n = 7;
    str parity = if (n % 2 == 0) { "even" } else { "odd" };     // if is an expression

    i32 squared =                     // a block is an expression: its last line is its value
    {
        i32 t = n * n;
        t
    };

    i32 clamped = if (n < 0)
    {
        0
    }
    else if (n > 5)
    {
        5
    }
    else
    {
        n
    };

    printf("%v is %v; squared %v; clamped %v\n", n, parity, squared, clamped);
}
```

```
7 is odd; squared 49; clamped 5
```

An `if` used as a value must have an `else`, and both branches must have the
same type. An `if` without `else` is a statement, and so is any `if` whose
branches end in semicolons. Notice the `;` after the closing brace when the
`if` or block is the right-hand side of a declaration: the declaration is a
statement, and the statement ends there. An `if` standing on its own needs
no semicolon after its brace.

A condition is always in parentheses, and a branch is always in braces, even
when it is one line. There is no dangling-else ambiguity and no way to add a
second statement to a branch by mistake.

## Loops

```
import std;

fn main()
{
    // while: the condition in parentheses, the body in braces, always.
    u32 n = 27;
    u32 steps = 0;
    while (n != 1)
    {
        n = if (n % 2 == 0) { n / 2 } else { 3 * n + 1 };
        steps += 1;
    }
    printf("27 reaches 1 in %v steps\n", steps);

    // for: C's counted loop.
    for (i32 i = 10; i > 0; i -= 3)
    {
        printf("%v ", i);
    }
    printf("\n");

    // foreach over a range: from the first number up to, not including, the second.
    i32 sum = 0;
    foreach (k in 1..11: i32)
    {
        if (k == 4)
        {
            continue;                 // skip 4
        }
        if (k == 8)
        {
            break;                    // stop before 8
        }
        sum += k;
    }
    printf("sum %v\n", sum);

    // foreach over an array, with the position.
    auto names = ["ada", "lin", "kay"];
    foreach (i, name in names)
    {
        printf("%v: %v\n", i, name);
    }
}
```

```
27 reaches 1 in 111 steps
10 7 4 1 
sum 24
0: ada
1: lin
2: kay
```

`while` and `for` are C's. `for (;;)` loops until a `break`; so does
`while (true)`, and the language knows that a `while (true)` with no `break`
never finishes, so a function may end with one and need no value after it.

`foreach` walks a collection or a range. A range `lo..hi` counts from `lo` up
to but not including `hi`, so `0..n` runs `n` times. Its numbers take the
bounds' type, and two bare literals are `usize`, which is why `1..11: i32`
above gives the bound a type: the sum is an `i32`. A second name before the
element gives its position, counted from zero. There is no `++`; write
`+= 1`.

Chapter 7 says how `foreach` borrows the collection it walks, and chapter 10
shows it over vectors and maps.

## Scopes

A block opens a scope. Everything declared in it is gone when it ends:

```
import std;

fn main()
{
    i32 x = 1;
    {
        i32 y = x + 1;                // y lives until this block ends
        i32 x = 100;                  // an inner x hides the outer one here
        printf("inner %v %v\n", x, y);
    }
    printf("outer %v\n", x);          // the outer x, untouched
    printf("%v\n", y);                // ✗ y's block has ended
}
```

```
error[diag.unbound-name]: nothing named `y` is declared here
  --> scopes.cb:12:20
   |
12 |     printf("%v\n", y);                // ✗ y's block has ended
   |                    ^
   = repair: declare it, fix the spelling, or add an `import` (`import std;` for the standard library, `CHG-0033`)
```

So far this is only a rule about names. In the next chapter it becomes a rule
about *things*: when a block ends, every value it owns is destroyed, in the
reverse of the order it was made, and that moment is where files are closed
and memory is freed. A block is therefore a tool, not just a grouping. You
will open one to bound how long something lives, and close it to let the
thing go.

## Evaluation order

Everything is evaluated left to right: the left operand before the right,
a function before its arguments, arguments in the order written. `&&` and
`||` stop as soon as the answer is known. The order is part of the language,
so a program may rely on it, though readers will thank you for not making
them work it out.

The operators and their precedence are listed in appendix A. The only one
that differs from C's habits is the one mentioned in chapter 4: the bitwise
operators bind tighter than comparisons.
