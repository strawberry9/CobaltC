// D-0286: values between functions in a decided program, which travel with
// no object: a by-value argument or a result is handed over as its bytes,
// the sender's record of it ended (`cb_forget_obj`), and the receiver
// takes it objectless or gives it an object of its own (`cb_new`).

use super::*;

impl<'g, 'a> Fx<'g, 'a> {
    // A resource argument leaves for a by-value parameter: in flight
    // (`cb_send`); in a decided program, as its value alone, its object's
    // record ended here (an inert value, `inert_c`, has none).
    pub(super) fn give_arg(&mut self, t: &V, what: &str) -> R<()> {
        match &t.obj {
            Some(o) if self.g.decided => self.emit(&format!("cb_forget_obj({});", o)),
            Some(o) => self.emit(&format!("cb_send({});", o)),
            None if self.g.decided && self.inert_c.contains(&t.c) => {}
            None => return internal(&format!("a resource argument without an object: {} (`{}`)", what, t.c)),
        }
        Ok(())
    }

    // D-0286: the call `e`, if it is one, gives its resource result to the
    // consumer about to lower it with no object (`finish_call`). Unless
    // nothing is evaluated between the call and the consumer (`last`), only
    // a result whose end frees memory alone (`objectless_type`): a fault in
    // a later argument or field leaves its bytes unfreed, which no code of
    // the program sees.
    pub(super) fn arm_inert(&mut self, e: &Expr, last: bool) {
        if !self.g.decided {
            return;
        }
        let ExprKind::Call(_, cargs) = &strip_parens(e).kind else { return };
        self.inert_result = Some((cargs.as_ptr() as usize, last));
    }

    // D-0286: `e` builds an aggregate itself (a struct literal, or a
    // variant), which `objectless` makes with no object.
    pub(super) fn literal_expr(&self, e: &Expr) -> bool {
        match &strip_parens(e).kind {
            ExprKind::StructLit(..) => true,
            ExprKind::Path(segs, _) => !(segs.len() == 1 && self.lookup(&segs[0]).is_some()) && self.variant_of(segs).is_some(),
            ExprKind::Call(c, _) => match &c.kind {
                ExprKind::Path(segs, _) => !(segs.len() == 1 && self.lookup(&segs[0]).is_some()) && !self.g.items.fns.contains_key(&segs.join("::")) && self.variant_of(segs).is_some(),
                _ => false,
            },
            _ => false,
        }
    }

    // D-0286: `e` is `x.f…` for an objectless local `x`.
    pub(super) fn objectless_root(&self, e: &Expr) -> bool {
        let mut e = strip_parens(e);
        while let ExprKind::Field(a, _) = &e.kind {
            e = strip_parens(a);
        }
        path_name(e).and_then(|n| self.lookup(n)).map_or(false, |b| matches!(b.kind, BindKind::Local) && self.objectless_c.contains(&b.c))
    }

    // A call's resource result `t` arrives: received from flight; in a
    // decided program, given an object where it lands. The object's name.
    pub(super) fn take_result(&mut self, t: &str, ty: &Type, line: usize) -> R<String> {
        let o = self.g.fresh("o");
        if self.g.decided {
            let tid = self.tid(ty)?;
            let loc = self.loc(line);
            self.emit(&format!("uint64_t {} = cb_new(&{}, {}u, {});", o, t, tid, loc));
        } else {
            self.emit(&format!("uint64_t {} = cb_recv(&{});", o, t));
        }
        Ok(o)
    }
}

// A native body's fixed text, made for a decided program (D-0286). Its
// parameters arrive with no object and its result leaves with none, so:
// an object received for a parameter (`cb_recv(&p_x)`) that the body only
// ends, sends on or moves (`cb_forget_obj`, `cb_send`, `cb_move_to`,
// `cb_raw_move_in`) is not made, and those uses go with it, as in the
// natives' `_raw` forms; one the body uses otherwise is made where it
// would have been received (`cb_new`, of the parameter's type, `tids`).
// Every other send ends its object's record instead, and an object made
// only to be ended at once is not made (`ground_sends`). A receive of
// anything else is an error: the text is one this rule does not know.
// A removed statement leaves an empty one where it stood, so that an `if`
// it was the body of keeps one.
pub(super) fn ground_native(text: &str, tids: &[(String, u32)]) -> Result<String, String> {
    let mut s = text.to_string();
    // A result received and ended at once (a call's, in a `__raw` form).
    while let Some(i) = s.find("cb_forget_obj(cb_recv(&") {
        let Some(j) = s[i..].find("));") else { return Err("an unclosed receive".to_string()) };
        s.replace_range(i..i + j + 3, ";");
    }
    let mut recvs: Vec<(String, String)> = Vec::new();
    let mut at = 0;
    while let Some(k) = s[at..].find("uint64_t ") {
        let i = at + k;
        at = i + 9;
        let rest = &s[i + 9..];
        let Some(eq) = rest.find(" = cb_recv(&") else { continue };
        let name = &rest[..eq];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let after = &rest[eq + 12..];
        let Some(close) = after.find(");") else { continue };
        recvs.push((name.to_string(), after[..close].to_string()));
    }
    for (x, p) in &recvs {
        let decl = format!("uint64_t {} = cb_recv(&{});", x, p);
        let ends = [format!("cb_forget_obj({});", x), format!("cb_send({});", x)];
        let moves = [format!("cb_move_to({}, ", x), format!("cb_raw_move_in({}, ", x)];
        // Every mention of `x` is one of these statements (each written
        // `call(x…`, so the mention is preceded by the call's name).
        let stmt_at = |i: usize, st: &str| {
            let k = st.find('(').map_or(0, |k| k + 1);
            i >= k && s[i - k..].starts_with(st)
        };
        let mut only = true;
        for i in ident_positions(&s, x) {
            let in_decl = i >= 9 && s[i - 9..].starts_with(&decl);
            if !(in_decl || ends.iter().any(|e| stmt_at(i, e)) || moves.iter().any(|m| stmt_at(i, m))) {
                only = false;
                break;
            }
        }
        if only {
            s = s.replace(&decl, ";");
            for e in &ends {
                s = s.replace(&e[..], ";");
            }
            for m in &moves {
                while let Some(i) = s.find(&m[..]) {
                    let Some(j) = s[i..].find(");") else { return Err(format!("an unclosed `{}`", m)) };
                    s.replace_range(i..i + j + 2, ";");
                }
            }
        } else {
            let Some((_, tid)) = tids.iter().find(|(n, _)| n == p) else { return Err(format!("a receive of `{}`, no parameter", p)) };
            s = s.replace(&decl, &format!("uint64_t {} = cb_new(&{}, {}u, CB_NOLOC, 0);", x, p, tid));
        }
    }
    if s.contains("cb_recv(") {
        return Err("a native receives what is no parameter".to_string());
    }
    let s = ground_sends(&s);
    Ok(tidy(&s))
}

// Text made for a decided program that sends what it made (a helper's
// result): each send ends its object's record instead, and an object made
// only to be ended at once is not made.
pub(super) fn ground_sends(text: &str) -> String {
    let mut s = text.to_string();
    let mut at = 0;
    while let Some(k) = s[at..].find("cb_send(") {
        let i = at + k;
        s.replace_range(i..i + 8, "cb_forget_obj(");
        at = i + 14;
    }
    // `uint64_t o = cb_new(&v, Tu, CB_NOLOC, 0);` then `cb_forget_obj(o);`
    // with nothing between: `o` names a record already ended after that.
    let mut at = 0;
    while let Some(k) = s[at..].find("uint64_t ") {
        let i = at + k;
        at = i + 9;
        let rest = &s[i + 9..];
        let Some(eq) = rest.find(" = cb_new(&") else { continue };
        let name = rest[..eq].to_string();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let Some(close) = rest.find(");") else { continue };
        let end = i + 9 + close + 2;
        let forget = format!("cb_forget_obj({});", name);
        let gap = s[end..].len() - s[end..].trim_start().len();
        if !s[end + gap..].starts_with(&forget) {
            continue;
        }
        let stop = end + gap + forget.len();
        s.replace_range(i..stop, ";");
        at = i;
    }
    tidy(&s)
}

// The positions of `x` as a whole identifier in `s`.
fn ident_positions(s: &str, x: &str) -> Vec<usize> {
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = s[at..].find(x) {
        let i = at + k;
        let j = i + x.len();
        if (i == 0 || !word(b[i - 1])) && (j >= b.len() || !word(b[j])) {
            out.push(i);
        }
        at = j;
    }
    out
}

// The empty statements the rewrites left: a line of one goes, and a run of
// them within a line becomes one.
fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.split_inclusive('\n') {
        let body = line.trim_end_matches('\n');
        if !body.trim().is_empty() && body.trim().chars().all(|c| c == ';' || c == ' ') {
            continue;
        }
        let mut l = body.to_string();
        while l.contains("; ;") {
            l = l.replace("; ;", ";");
        }
        while l.contains(";;") && !l.contains("for (") {
            l = l.replace(";;", ";");
        }
        out.push_str(&l);
        if line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
