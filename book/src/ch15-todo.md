# A larger program: a to-do list

This chapter builds a complete command-line tool and reads it from top to
bottom. It keeps a to-do list in a text file: you add tasks, list them, mark
them done, and remove them. It is a hundred and eighty lines long and uses
the language the way the previous chapters taught it, so the point here is
less any one feature than how they fit together: where the errors go, where
the modules go, how the file is kept honest, and how the command line is
read.

## What it does

```
$ todo --file $TMP/todo.txt list
nothing to do
$ todo --file $TMP/todo.txt add buy milk
added task 1
$ todo --file $TMP/todo.txt add write chapter 15
added task 2
$ todo --file $TMP/todo.txt add call the bank
added task 3
$ todo --file $TMP/todo.txt list
 1. [ ] buy milk
 2. [ ] write chapter 15
 3. [ ] call the bank
$ todo --file $TMP/todo.txt done 2
done: write chapter 15
$ todo --file $TMP/todo.txt remove 1
removed: buy milk
$ todo --file $TMP/todo.txt list
 1. [x] write chapter 15
 2. [ ] call the bank
$ todo --file $TMP/todo.txt done 9
todo: there is no task 9
$ todo --file $TMP/todo.txt done two
todo: 'two' is not a task number
$ todo --file $TMP/todo.txt frobnicate
todo: unknown command 'frobnicate'
$ todo --help
Usage: todo [--file PATH] COMMAND [ARGS]

Options:
  -h, --help       show this help
  -f, --file PATH  the list file (default: todo.txt)
```

Each of these runs is part of the book's tests, in that order, against one
file in a temporary directory; the error lines go to standard error and the
failing runs exit with status 1, or 2 for a usage mistake.

## The program

```
// todo: a to-do list kept in a text file.
//
//   todo [--file PATH] add TEXT...     add a task
//   todo [--file PATH] list            show the tasks
//   todo [--file PATH] done N          mark task N as done
//   todo [--file PATH] remove N        delete task N
//
// The file holds one task per line: "[ ] text" or "[x] text".

import std;

// Every way the program can fail, in one place.
enum TodoError
{
    File(FileError),                  // the list could not be read or written
    BadLine(usize),                   // the file has a line that is not a task
    NoSuchTask(usize),                // a number no task has
    Usage(String),                    // the command line made no sense
}

fn TodoError::text(ref<TodoError, shared> e) : String
{
    match (e)
    {
        File(f)       : String::from_str(FileError::text(f)),
        BadLine(n)    : sprintf("line %v of the list is not a task", *n),
        NoSuchTask(n) : sprintf("there is no task %v", *n),
        Usage(m)      : String::clone(m),
    }
}

// Reading and writing the list file. Nothing else touches the file.
module store
{
    import std;

    export struct Task
    {
        export String text;
        export bool done;
    }

    export fn load(StringView path) : Result<Vec<Task>, TodoError>
    {
        Vec<Task> tasks = Vec::new();
        String text = match (read_file(path))
        {
            Ok(t)         : t,
            Err(NotFound) : { return Ok(tasks); },      // no file yet: an empty list
            Err(e)        : { return Err(File(e)); },
        };
        foreach (i, line in String::lines(&text))
        {
            if (StringView::len(line) == 0)
            {
                continue;
            }
            if (Some(rest) = StringView::strip_prefix(line, "[x] "))
            {
                Vec::push(&mut tasks, Task { .text = String::from_view(rest), .done = true });
            }
            else if (Some(rest) = StringView::strip_prefix(line, "[ ] "))
            {
                Vec::push(&mut tasks, Task { .text = String::from_view(rest), .done = false });
            }
            else
            {
                return Err(BadLine(i + 1));
            }
        }
        Ok(tasks)
    }

    export fn save(StringView path, ref<Vec<Task>, shared> tasks) : Result<void, TodoError>
    {
        String out = String::new();
        foreach (t in tasks)
        {
            String::appendf(&mut out, "%v %v\n", if (t.done) { "[x]" } else { "[ ]" }, &t.text);
        }
        match (write_file(path, &out[0..$]))
        {
            Ok(_)  : Ok(()),
            Err(e) : Err(File(e)),
        }
    }
}

import store::Task;

// The task number given on the command line, checked against the list.
fn task_index(ref<Vec<String>, shared> args, ref<Vec<Task>, shared> tasks) : Result<usize, TodoError>
{
    if (Vec::len(args) != 2)
    {
        return Err(Usage(sprintf("%v takes one task number", &args[0])));
    }
    usize n = match (String::parse<usize>(&args[1]))
    {
        Ok(n)  : n,
        Err(_) : { return Err(Usage(sprintf("'%v' is not a task number", &args[1]))); },
    };
    if (n == 0 || n > Vec::len(tasks))
    {
        return Err(NoSuchTask(n));
    }
    Ok(n - 1)
}

fn run(StringView path, Vec<String> args) : Result<void, TodoError>
{
    Vec<Task> tasks = store::load(path)?;
    match (args[0])
    {
        "add"    :
        {
            if (Vec::len(&args) < 2)
            {
                return Err(Usage(String::from_str("add needs the task's text")));
            }
            Vec::push(&mut tasks, Task { .text = String::join(&args[1..$], " "), .done = false });
            store::save(path, &tasks)?;
            printf("added task %v\n", Vec::len(&tasks));
        },
        "list"   :
        {
            if (Vec::len(&tasks) == 0)
            {
                printf("nothing to do\n");
            }
            foreach (i, t in &tasks)
            {
                printf("%2d. [%v] %v\n", i + 1, if (t.done) { "x" } else { " " }, &t.text);
            }
        },
        "done"   :
        {
            usize i = task_index(&args, &tasks)?;
            tasks[i].done = true;
            store::save(path, &tasks)?;
            printf("done: %v\n", &tasks[i].text);
        },
        "remove" :
        {
            usize i = task_index(&args, &tasks)?;
            Task removed = Vec::remove(&mut tasks, i);
            store::save(path, &tasks)?;
            printf("removed: %v\n", &removed.text);
        },
        _        : { return Err(Usage(sprintf("unknown command '%v'", &args[0]))); },
    }
    Ok(())
}

fn main() : u8
{
    Args a = match (Args::new("todo [--file PATH] COMMAND [ARGS]"))
    {
        Ok(a)  : a,
        Err(_) : { eprintf("todo: an argument is not UTF-8\n"); return 2; },
    };
    bool help = Args::flag(&mut a, "help", "h", "show this help");
    String path = Option::unwrap_or(
        Args::option(&mut a, "file", "f", "PATH", "the list file (default: todo.txt)"),
        String::from_str("todo.txt"),
    );
    Vec<String> rest = match (Args::finish(&mut a))
    {
        Ok(r)  : r,
        Err(m) : { eprintf("todo: %v\n", &m); return 2; },
    };
    if (help || Vec::len(&rest) == 0)
    {
        printf("%v", &Args::help(&a));
        return if (help) { 0 } else { 2 };
    }
    match (run(&path[0..$], rest))
    {
        Ok(_)  : 0,
        Err(e) :
        {
            eprintf("todo: %v\n", &TodoError::text(&e));
            1
        },
    }
}
```

## The errors come first

The program begins by naming every way it can fail, in one enum. Two of
the variants wrap failures that come from elsewhere: `File` carries the
library's `FileError`, and `Usage` carries a message built where the
mistake was found. The other two are the program's own. Having one error
type means `?` flows through the whole program without conversion, and
`TodoError::text` is the single place where an error becomes a sentence for
a person. `main` prints that sentence and chooses an exit status; nothing
else in the program prints an error.

The variant `File` shares its name with a type that `std` exports, the
open-file handle. That is no trouble: where a value is written, `File(e)`
can only mean the variant, since a type is not a value, and where a type is
written, `File` is `std`'s. The language keeps the two positions apart.

## The file lives in a module

Everything that reads or writes the list file is in `module store`, and
the rest of the program sees two functions, `load` and `save`, and the
`Task` struct. If the file format changes, this module changes and nothing
else does.

`load` turns a missing file into an empty list, since the first `add` on a
new machine should not fail; every other file error is wrapped and returned.
The `match` on `read_file` has three arms, and the middle one shows a
pattern inside a pattern: `Err(NotFound)` matches only that variant of the
error, and the `Err(e)` after it catches the rest. Then each line is
classified with `strip_prefix`, which returns the remainder of the line as
`Some` when it begins with the given text. A line that is neither form is an
error that names its line number, so a hand-edited file that goes wrong says
where.

`save` builds the whole file in a `String` with `appendf` and writes it in
one call. Writing the whole file every time is the simplest thing that is
correct, and for a to-do list it is also fast enough. The `match` at the end
converts the library's error into the program's; `Ok(_)  : Ok(())` is how a
function with nothing to return reports success.

The module writes `import std;` for itself, because an import acts only in
the module that contains it. The `TodoError` variants, by contrast, need no
import inside `store`: a bare variant name is looked up in the enclosing
modules, and `File` and `BadLine` are found at the root.

## Commands are a `match` on text

`run` loads the list, does one command, and saves if anything changed. The
command is the first argument, and a `match` on it with text patterns is
the whole dispatch: four commands and a catch-all for anything else. Each
arm is a block, because each has statements to run; the `match` as a whole
is a statement and `run` finishes with `Ok(())`.

Two commands take a task number, and `task_index` checks it once for both:
the right number of arguments, a number at all, and a number some task has.
It returns the zero-based index that the vector wants, and `?` at each call
site returns its error unchanged. The people-facing numbers start at 1,
which is the kind of small fact worth keeping in exactly one function.

`add` joins the remaining arguments with spaces, so `todo add buy milk`
needs no quotes. `remove` uses `Vec::remove`, which hands back the removed
`Task` so that its text can be printed after it has left the list. Notice
that `run` takes `args` by value: it is the last user of the vector, and
taking ownership lets it slice and index the vector freely, with no caller
to give it back to.

## The command line

`main` uses the library's `Args` to read the options. Each `flag` and
`option` call declares an option, takes it out of the arguments, and adds a
line to the help text, so the help is never out of date with the options.
`Args::finish` returns what is left, the command and its arguments, or an
error naming an option nobody declared. With no command at all, the program
prints its usage and exits with 2, the conventional status for a usage
mistake; `--help` prints the same text and exits with 0, because the user
asked for it.

The last `match` is where the program's one error type pays off. Whatever
went wrong, anywhere in `run` or in `store`, arrives here as a `TodoError`,
is printed once, and becomes exit status 1.

## What to notice

The program has no `unsafe` block, so every guarantee in this book holds of
all of it. It has no exceptions and no error codes, and yet every failure
is handled: the compiler would have rejected a `Result` that was dropped, a
`match` with a missing arm, or a use of the list after it was moved. It
has no memory management, and yet every `String` and `Vec` is freed at a
moment you could point to. And it is organised so that each kind of
knowledge, the file format, the numbering, the error messages, the
command-line syntax, is in one place.

Some things to try. Add a `clear` command that removes every task that is
done. Add a `--all` flag to `list` that hides done tasks unless it is
given. Make `add` refuse an empty task. Each of these touches one or two
places, which is the test of whether a program is well arranged.
