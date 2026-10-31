# Modules, files and tests

A program of any size needs a way to group its declarations, hide the ones
that are nobody else's business, and spread itself over several files. In
CobaltC all three are done by **modules**, which add no new semantics: every
rule about ownership, borrowing and types holds unchanged across a module
boundary. This chapter also covers `unittest`, since tests live beside the
code they test, inside its modules.

## A module hides what it does not export

```
import std;

module bank
{
    import std;                       // each module imports what it uses

    export struct Account
    {
        export u32 id;                // visible outside the module
        i64 balance;                  // private: only this module's functions reach it
    }

    export fn Account::open(u32 id, i64 deposit) : Account
    {
        assert(deposit >= 0, "an account opens with a deposit, not a debt");
        Account { .id = id, .balance = deposit }
    }

    export fn Account::balance(ref<Account, shared> a) : i64
    {
        a.balance
    }

    export fn Account::withdraw(ref<Account, exclusive> a, i64 amount) : bool
    {
        if (amount > a.balance)
        {
            return false;             // the invariant: a balance never goes below zero
        }
        a.balance -= fee(amount);
        true
    }

    fn fee(i64 amount) : i64          // private: a helper nobody outside needs
    {
        amount + amount / 100
    }
}

fn main()
{
    bank::Account acct = bank::Account::open(7, 1000);
    printf("account %v holds %v\n", acct.id, bank::Account::balance(&acct));
    printf("withdraw 5000: %v\n", bank::Account::withdraw(&mut acct, 5000));
    printf("withdraw 300: %v\n", bank::Account::withdraw(&mut acct, 300));
    printf("now %v\n", bank::Account::balance(&acct));
}
```

```
account 7 holds 1000
withdraw 5000: false
withdraw 300: true
now 697
```

`module name { ... }` encloses declarations. Everything in it is private
unless marked `export`: usable from inside the module and from modules nested
in it, and from nowhere else. A struct's fields have their own visibility,
field by field, so a module can expose a type while keeping control of what
goes inside it. Here `balance` is private, so the only way to change it is
through `withdraw`, and the module's promise that a balance never goes
negative is kept in one place.

Outside the module, its items are reached by their qualified names,
`bank::Account::open`. A private item reached from outside is rejected:

```
import std;

module bank
{
    export struct Account
    {
        export u32 id;
        i64 balance;
    }
}

fn main()
{
    bank::Account a = bank::Account { .id = 1, .balance = 50 };   // ✗ balance is private
    printf("%v\n", a.id);
}
```

```
error[diag.name-not-visible]: field `balance` of `bank::Account` is not exported
  --> private.cb:14:23
   |
14 |     bank::Account a = bank::Account { .id = 1, .balance = 50 };   // ✗ balance is private
   |                       ^^^^
   = repair: mark `export` or move the reference
```

There are no read-only fields. A field that others may read but not change
is a private field with an exported function that reads it, as `balance`
is. That is also where the module's invariant is enforced, so the two needs
are served by one mechanism.

## `import`

`import bank::Account;` lets `Account` be used unqualified in the module that
writes it. `import bank;` lets every exported item of `bank` be used
unqualified. Neither copies or renames anything; they only extend name
lookup. That is what `import std;` does at the top of every program, and it
is why each module writes its own `import std;`: an import acts only in the
module that contains it.

A name is looked up in a fixed order: the module's own declarations first,
then its single-name imports, then its module imports, then the enclosing
modules. So a program's own `fn max` wins over `std`'s, which stays
reachable as `std::max`. When two imported modules export the same name,
using it unqualified is an error, and qualifying it resolves the question;
importing both is fine, so a library that gains an export never breaks a
program that does not use it. The tools' `--warn-shadow` option notes each
name of your program that hides an imported one, for a program that would
rather know.

## A module in a file of its own

A module's body can be a separate file: `module name "path.cb";` means
exactly `module name { ... }` with that file's declarations as the body.
The file's own top level is anonymous; the file that declares the module
names it. Paths are relative to the file that writes them, so a library
moves as a unit.

A library split across files is wired together with one more form:
`export import`, which passes on what a module imports as its own items. A
root file declares the parts and re-exports what users should see:

```
// inventory.cb: the library's root. `import inventory;` brings in what it re-exports.
export module items "items.cb";
export module report "report.cb";

export import inventory::items;       // every exported item of items, as inventory's own
export import inventory::report::print_all;   // one function from report
```

```
// items.cb: the body of module `items`. Nothing here names the module;
// the file that declares it does.
import std;

export struct Item
{
    export String name;
    export u32 count;
}

export fn Item::new(str name, u32 count) : Item
{
    Item { .name = String::from_str(name), .count = count }
}

export fn describe(ref<Item, shared> it) : String
{
    sprintf("%v x %v", it.count, &it.name)
}
```

```
// report.cb: another module, which uses `items` through the root's declaration.
import std;
import inventory::items;

export fn print_all(ref<Vec<Item>, shared> stock)
{
    u32 total = 0;
    foreach (it in stock)
    {
        printf("  %v\n", &inventory::items::describe(it));   // qualified: `std` exports a `describe` too
        total += it.count;
    }
    printf("  %v items in all\n", total);
}
```

The program that uses it declares the root as a module and imports it:

```
import std;

module inventory "inventory/inventory.cb";    // a module whose body is a file
import inventory;

fn main()
{
    Vec<Item> stock = Vec::new();             // inventory::items::Item, through the re-export
    Vec::push(&mut stock, Item::new("bolts", 40));
    Vec::push(&mut stock, Item::new("nuts", 35));
    printf("stock:\n");
    print_all(&stock);
    printf("%v\n", &inventory::items::describe(&stock[0]));   // the full path works too
}
```

```
stock:
  40 x bolts
  35 x nuts
  75 items in all
40 x bolts
```

`coby files.cb` runs the whole thing and `cobc files.cb` builds it: the root
file names every other file, so there is no project file and no build
script. A path that names no file, a file loaded twice under two names, and
a cycle of files are each rejected before anything runs.

## Tests beside the code

A `unittest` block, at the top level of a file or a module, is a test. It
sees the module's private items, and it is checked every time the program
is, so a test that no longer compiles stops the build; it runs only when
asked.

```
import std;

fn parse_port(StringView s) : Option<u16>
{
    match (StringView::parse<u32>(s))
    {
        Ok(n)  : if (n > 65535) { None } else { Some(narrow<u16>(n)) },
        Err(_) : None,
    }
}

unittest "parse_port accepts the whole range"
{
    assert(parse_port("0") == Some(0));
    assert(parse_port("65535") == Some(65535));
}

unittest "parse_port rejects what is not a port"
{
    assert(parse_port("65536") == None);
    assert(parse_port("x") == None);
    assert(parse_port("") == None);
}

fn main()
{
    printf("%v\n", parse_port("8080") == Some(8080));
}
```

`coby ports.cb` runs `main` as always. `coby --unittest ports.cb` runs the
tests instead, each in a thread of its own, so a failing `assert` in one is
reported and the next still runs:

```
test ports.cb:12 parse_port accepts the whole range ... ok
test ports.cb:18 parse_port rejects what is not a port ... ok
2 tests: 2 passed, 0 failed
```

`cobc --unittest --run ports.cb` does the same with the compiled program.
The exit status is 1 when a test failed, so a test run can gate a build,
and `--unittest-filter TEXT` runs only the tests whose names contain `TEXT`.
A file that holds only a library and its tests needs no `main`.

## Organising a program

Start with everything private and export what callers need; a module's
exports are its interface, and a narrow one is easy to keep true. Put one
module in one file once a file grows past a screen or two. Give a library a
root file that re-exports its public face, so that it is used with one
`import` and can be rearranged inside without touching the programs that
use it. And write the tests in the module, where they can see what they
test and where a change to the code cannot leave them behind.
