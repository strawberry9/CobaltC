// D-0303 (`spec/17` `rule.module.retired`): the standing of every declared
// name, and the check a use of one passes.
//
// `modres` builds the table once per program, as it builds its module tree,
// and checks each path it resolves; the type checker checks what only it
// can see: a field (access, struct literal, destructuring, struct pattern)
// and a variant named in a pattern. A use inside a declaration that is
// itself retired (its own body; an associated function of a deprecated
// type; an item of a deprecated module), inside a function the front end
// derived, or inside the `unittest` that immediately follows a retired
// item does not count.
//
// Under `--allow-deprecated` (`COBALTC_ALLOW_DEPRECATED`) a deprecated use
// is a warning, collected here and printed after the program is accepted;
// a removed use is an error whatever the flag says.

use crate::ast::Retired;
use crate::fxhash::{HashMap, HashSet};
use std::cell::RefCell;

#[derive(Default)]
pub struct Table {
    // Qualified item key (`std::String::copy_str`, `m::T`, `m`, `E::V`
    // for a variant) to its standing, a module's or a type's carried to
    // what it declares.
    pub items: HashMap<String, Retired>,
    // (struct key, field name) to the field's standing.
    pub fields: HashMap<(String, String), Retired>,
    // Functions in which no use counts (qualified keys).
    pub exempt: HashSet<String>,
}

thread_local! {
    static TABLE: RefCell<Table> = RefCell::new(Table::default());
    static NOTES: RefCell<Vec<(usize, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn install(t: Table) {
    TABLE.with(|c| *c.borrow_mut() = t);
    NOTES.with(|n| n.borrow_mut().clear());
}

pub fn allowed() -> bool {
    std::env::var_os("COBALTC_ALLOW_DEPRECATED").map_or(false, |v| !v.is_empty() && v != "0")
}

pub fn is_exempt(fn_key: &str) -> bool {
    TABLE.with(|c| c.borrow().exempt.contains(fn_key))
}

pub fn any() -> bool {
    TABLE.with(|c| {
        let t = c.borrow();
        !t.items.is_empty() || !t.fields.is_empty()
    })
}

// The standing of `key` or of the nearest retired module or type its path
// passes through (`a::b::c` is a use of `a::b` and of `a`).
fn standing(t: &Table, key: &str) -> Option<Retired> {
    if let Some(r) = t.items.get(key) {
        return Some(r.clone());
    }
    let mut k = key;
    while let Some((head, _)) = k.rsplit_once("::") {
        if let Some(r) = t.items.get(head) {
            return Some(r.clone());
        }
        k = head;
    }
    None
}

// The verdict on a use of the item `key`, written `shown`, at `line`, in a
// function where uses count unless `exempt`.
pub fn check_item(key: &str, shown: &str, line: usize, exempt: bool) -> Result<(), String> {
    let r = TABLE.with(|c| standing(&c.borrow(), key));
    verdict(r, &format!("`{}`", shown), line, exempt)
}

pub fn check_field(struct_key: &str, field: &str, line: usize, exempt: bool) -> Result<(), String> {
    let r = TABLE.with(|c| c.borrow().fields.get(&(struct_key.to_string(), field.to_string())).cloned());
    let shown = struct_key.strip_prefix("std::").unwrap_or(struct_key);
    verdict(r, &format!("the field `{}` of `{}`", field, shown), line, exempt)
}

fn verdict(r: Option<Retired>, what: &str, line: usize, exempt: bool) -> Result<(), String> {
    match r {
        None | Some(Retired::Live) => Ok(()),
        Some(Retired::Removed(m)) => Err(format!("{}@{}", crate::modres::named("diag.removed-use", format!("{} was removed: {}", what, m)), line)),
        Some(Retired::Deprecated(_)) if exempt => Ok(()),
        Some(Retired::Deprecated(m)) => {
            let text = format!("{} is deprecated: {}", what, m);
            if allowed() {
                NOTES.with(|n| n.borrow_mut().push((line, format!("warning[diag.deprecated-use]: {}", text))));
                Ok(())
            } else {
                Err(format!("{}@{}", crate::modres::named("diag.deprecated-use", text), line))
            }
        }
    }
}

/// The warnings `--allow-deprecated` collected, in source order, each once.
pub fn take_notes() -> Vec<(usize, String)> {
    let mut v = NOTES.with(|n| std::mem::take(&mut *n.borrow_mut()));
    v.sort();
    v.dedup();
    v
}
