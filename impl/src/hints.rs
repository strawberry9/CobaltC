// CHG-0445: what a diagnostic adds when the text it rejects is a form from
// another language, or a name spelt nearly as one that exists: the CobaltC
// form, or the name meant. Only the message changes (the registry: a
// message's wording is not normative); the twin is
// impl/selfhost/front/hints.cb, and both give the same text.

/// The optimal string alignment distance between `a` and `b` (insertions,
/// deletions, substitutions and swaps of two neighbours, each 1), or a
/// number above `limit` once it is certain to exceed it.
pub fn distance(a: &str, b: &str, limit: usize) -> usize {
    // Bytes, as the twin compares them (the names are ASCII).
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > limit {
        return limit + 1;
    }
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = v;
        }
    }
    d[n][m]
}

/// The one candidate nearest to `name`: within one edit for a name of four
/// characters or fewer, two otherwise, fewer edits than half the name's
/// length (so no name of one or two characters is offered one: `y` for
/// `x`), and nearer than every other candidate (a tie names none). A
/// candidate equal to `name` is not one.
pub fn nearest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let limit = (if name.len() <= 4 { 1 } else { 2 }).min((name.len().max(1) - 1) / 2);
    if limit == 0 {
        return None;
    }
    let mut best: Option<(usize, String)> = None;
    let mut tie = false;
    for c in candidates {
        if c == name || c.is_empty() || c.starts_with('$') || c.contains('$') {
            continue;
        }
        let d = distance(name, c, limit);
        if d > limit {
            continue;
        }
        match &best {
            Some((bd, bc)) if d == *bd && bc != c => tie = true,
            Some((bd, _)) if d < *bd => {
                best = Some((d, c.to_string()));
                tie = false;
            }
            None => best = Some((d, c.to_string())),
            _ => {}
        }
    }
    if tie {
        return None;
    }
    best.map(|(_, c)| c)
}

/// What a plain name that names nothing means in another language, as the
/// end of the message: `; …`.
pub fn plain_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "NULL" | "null" | "nil" | "nullptr" | "undefined" => "; there is no null: a value that may be absent is an `Option<T>`, and `None` where it is absent",
        "malloc" | "calloc" | "realloc" | "free" | "new" | "delete" => "; memory is not allocated by hand: a `Vec<T>` (`Vec::filled(n, 0)`), a `Box<T>` (`Box::new(x)`) or a struct holds it, and it is freed when its owner's scope ends",
        "len" | "length" | "size" | "sizeof" => "; a length is a function of the type: `Vec::len(&v)`, `StringView::len(&s)` (a `String` too), `slice_len(s)`; a type's size is `sizeof<T>()`",
        "println" | "print" | "puts" | "echo" | "console" | "cout" | "System" | "fmt" => "; printing is `printf(\"%v\\n\", x)` (`eprintf` for standard error)",
        "assert_eq" | "assert_ne" | "assertEquals" => "; write `assert(a == b)` (`assert(a != b)`), with a message after the condition if wanted",
        "format" => "; text is formatted by `sprintf(\"%v\", x)`, which gives a `String`",
        "to_string" | "toString" => "; a value's text is `sprintf(\"%v\", x)`; a copy of a view's text is `String::copy(v)`",
        _ => return None,
    })
}

/// What `T::name` that names nothing means in another language: the
/// function of `T` it is written as, when `T` has it.
pub fn fn_name(name: &str) -> &'static [&'static str] {
    match name {
        "length" | "size" | "count" | "len_of" => &["len"],
        "push_back" | "add" | "append_one" | "emplace_back" | "pushBack" => &["push"],
        "pop_back" => &["pop"],
        "includes" | "has" | "contains_key" | "exists" => &["contains"],
        "put" | "set" | "add_entry" => &["insert"],
        "from" | "new_from" | "of_str" => &["copy", "from_slice"],
        "to_string" | "toString" | "to_owned" => &["copy"],
        "erase" | "delete" | "remove_at" => &["remove"],
        "at" | "nth" => &["get"],
        "equals" | "eq_str" => &["eq"],
        "starts_with_str" | "startsWith" => &["starts_with"],
        "ends_with_str" | "endsWith" => &["ends_with"],
        _ => &[],
    }
}

/// What a type name that names nothing means in another language.
pub fn type_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "int" | "int32_t" | "long" | "short" | "unsigned" | "signed" | "char" | "size_t" | "ssize_t" | "uint8_t" | "uint16_t" | "uint32_t" | "uint64_t" | "int8_t" | "int16_t" | "int64_t" | "byte" | "uint" | "integer" => {
            "; an integer type names its width and sign: `i32`, `u8`, `u64`, `usize`, …; C's `int` is `i32`, `char` is `u8`, `size_t` is `usize`"
        }
        "float" | "double" | "float32" | "float64" => "; the floating types are `f32` and `f64` (C's `double` is `f64`)",
        "boolean" => "; the truth type is `bool`",
        "string" | "string_view" | "std" => "; owned text is a `String`, and a view of text (a parameter, a literal) a `StringView`",
        "let" | "var" | "val" => "; a local is declared with its type, `i32 x = 1;`, or with `auto x = 1;`",
        "goto" => "; there is no `goto`: leave a loop with `break` or `continue`, and a function with `return`",
        "List" | "ArrayList" | "vector" | "Array" => "; a growable array is a `Vec<T>`, a fixed one an `array<T, N>`",
        "Map" | "HashTable" | "Dictionary" | "dict" | "unordered_map" | "map" => "; a hash table is a `HashMap<K, V>`",
        "Set" | "unordered_set" | "set" => "; a set is a `HashSet<T>`",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        assert_eq!(distance("sqaure", "square", 2), 1);
        assert_eq!(distance("psh", "push", 1), 1);
        assert_eq!(distance("coutn", "count", 2), 1);
        assert_eq!(distance("abc", "abcdef", 2), 3);
    }

    #[test]
    fn nearest_is_unique() {
        assert_eq!(nearest("psh", ["push", "pop", "len"]).as_deref(), Some("push"));
        assert_eq!(nearest("ab", ["ac", "ad"]), None);
        assert_eq!(nearest("Vect", ["Vec", "String"]).as_deref(), Some("Vec"));
        assert_eq!(nearest("zzzzzz", ["push"]), None);
        assert_eq!(nearest("y", ["x", "yy"]), None);
        assert_eq!(nearest("ab", ["ac"]), None);
        assert_eq!(nearest("psh", ["push"]).as_deref(), Some("push"));
    }
}
