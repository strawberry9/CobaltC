// extern calls and the arena encoding of their values.

use super::*;

impl Interp {
    pub(super) fn call_extern(&mut self, ext: &Arc<ExternDecl>, args: &[Expr], subst: &HashMap<String, Type>) -> EvalOutcome {
        self.require_unsafe()?;
        let mut vals = Vec::new();
        for a in args {
            vals.push(self.eval_to_value(a, subst)?);
        }
        // `std::arg_bytes` (`rule.stdlib.args` `[Arg-Bytes]`): up to `len`
        // bytes of argument `i` into the arena; the argument's whole
        // length, or -1 when there is no argument `i`.
        if self.items.externs.get("std::arg_bytes").map_or(false, |e| Arc::ptr_eq(e, ext)) {
            if let (Some(Value::Int(_, i)), Some(Value::Rawptr(addr, _)), Some(Value::Int(_, len))) = (vals.get(0), vals.get(1), vals.get(2)) {
                self.extern_calls.push(("arg_bytes".into(), vec![*i, *addr as i128, *len]));
                let args = self.prog_args.clone();
                let n: i128 = match args.get(*i as usize) {
                    Some(a) => {
                        for (j, b) in a.iter().take(*len as usize).enumerate() {
                            self.arena.insert(addr + j as u64, *b);
                        }
                        a.len() as i128
                    }
                    None => -1,
                };
                return Ok(EvalResult::Val(Value::Int(IntTy::Isize, n)));
            }
        }
        // `std::file_read`, `std::file_write` (`rule.stdlib.file`): real
        // file access (`fileio`), the file's bytes to and from the arena.
        let is_std = |key: &str| self.items.externs.get(key).map_or(false, |e| Arc::ptr_eq(e, ext));
        // `std::stdout_flush` (D-0178): standard output's buffer written out.
        if is_std("std::stdout_flush") {
            self.extern_calls.push((ext.name.clone(), vec![]));
            let r: i128 = if crate::outbuf::flush().is_ok() { 0 } else { -1 };
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, r)));
        }
        if is_std("std::file_read") || is_std("std::file_write") {
            let read = is_std("std::file_read");
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let addr = |v: Option<&Value>| match v {
                Some(Value::Rawptr(a, _)) => *a,
                _ => 0,
            };
            let (pa, pn, ba, bn) = (addr(vals.get(0)), int(vals.get(1)), addr(vals.get(2)), int(vals.get(3)));
            let bytes_at = |me: &Self, a: u64, n: u64| -> Vec<u8> { (0..n).map(|j| *me.arena.get(&(a + j)).unwrap_or(&0)).collect() };
            let path = bytes_at(self, pa, pn);
            self.extern_calls.push((ext.name.clone(), vec![pa as i128, pn as i128, ba as i128, bn as i128]));
            let r: i128 = if read {
                match crate::fileio::read(&path) {
                    Ok(data) => {
                        for (j, b) in data.iter().take(bn as usize).enumerate() {
                            self.arena.insert(ba + j as u64, *b);
                        }
                        data.len() as i128
                    }
                    Err(c) => c as i128,
                }
            } else {
                let data = bytes_at(self, ba, bn);
                match crate::fileio::write(&path, &data) {
                    Ok(()) => 0,
                    Err(c) => c as i128,
                }
            };
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, r)));
        }
        // `std::event_op` (`rule.conc.channel`, D-0063): the event counts a
        // channel waits on, in `Shared`, where a waiting thread reads them.
        // `std::task_op` (D-0204): a contained failure's report (op 0) or
        // its diagnostic's name (op 1), copied to the arena.
        if is_std("std::task_op") {
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let (op, id) = (int(vals.get(0)), int(vals.get(1)));
            let (at, cap) = match vals.get(2) {
                Some(Value::Rawptr(a, _)) => (*a, int(vals.get(3))),
                _ => (0, 0),
            };
            let text: Vec<u8> = {
                let sh = self.shared.lock().unwrap();
                match sh.failures.get(id as usize) {
                    Some((_, shown)) if op == 0 => shown.clone().into_bytes(),
                    Some((raw, _)) => crate::diagnostics::split_location(raw).0.to_string().into_bytes(),
                    None => Vec::new(),
                }
            };
            if text.len() as u64 <= cap {
                for (j, b) in text.iter().enumerate() {
                    self.arena.insert(at + j as u64, *b);
                }
            }
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, text.len() as i128)));
        }
        if is_std("std::event_op") {
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let (op, id, seen) = (int(vals.get(0)), int(vals.get(1)), int(vals.get(2)));
            let r = match op {
                0 => {
                    let mut sh = self.shared.lock().unwrap();
                    sh.last_event += 1;
                    let id = sh.last_event;
                    sh.events.insert(id, 0);
                    id
                }
                1 => self.shared.lock().unwrap().events.get(&id).copied().unwrap_or(0),
                2 => {
                    // `[Channel-Deadlock]`: the main thread waits on a count
                    // no other thread is running to change -- checked when
                    // the wait starts and whenever the last other thread
                    // ends during it.
                    let is_main = self.current_thread == 0;
                    // Not when `[Deadlock]` (b) already made the main thread an
                    // end of a chain of waits: that fault comes first, whatever
                    // the other ends' contained faults leave running (D-0219).
                    let alone = move |s: &Shared| is_main && !s.deadlocked.contains(&0) && !s.thread_status.values().any(|t| matches!(t, ThreadStatus::Running));
                    self.wait_for(Wait::Event(id, seen), |s| s.events.get(&id) != Some(&seen) || alone(s))?;
                    let sh = self.shared.lock().unwrap();
                    if sh.events.get(&id) == Some(&seen) {
                        return Err(Flow::Fault("diag.channel-deadlock".into()));
                    }
                    sh.events.get(&id).copied().unwrap_or(0)
                }
                3 => {
                    let mut sh = self.shared.lock().unwrap();
                    let c = sh.events.entry(id).or_insert(0);
                    *c = c.wrapping_add(1);
                    *c
                }
                _ => {
                    self.shared.lock().unwrap().events.remove(&id);
                    0
                }
            };
            return Ok(EvalResult::Val(Value::Int(IntTy::Usize, r as i128)));
        }
        // `std::file_op`, `std::file_at` (`rule.stdlib.file-handle`, D-0054): the open-file
        // table of `fileio`, the bytes to and from the arena.
        let at = is_std("std::file_at");
        if at || is_std("std::file_op") {
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let (op, h) = (int(vals.get(0)), int(vals.get(1)));
            let (ba, n) = match vals.get(2) {
                Some(Value::Rawptr(a, _)) => (*a, int(vals.get(3))),
                other => (0, int(other)),
            };
            self.extern_calls.push((ext.name.clone(), vec![op as i128, h as i128, ba as i128, n as i128]));
            let input: Vec<u8> = if crate::fileio::takes_input(op) { (0..n).map(|j| *self.arena.get(&(ba + j)).unwrap_or(&0)).collect() } else { Vec::new() };
            let (r, out) = crate::fileio::call(op, h, &input, n);
            for (j, b) in out.iter().enumerate() {
                self.arena.insert(ba + j as u64, *b);
            }
            let ty = if at { IntTy::I64 } else { IntTy::Isize };
            return Ok(EvalResult::Val(Value::Int(ty, r as i128)));
        }
        // `std::path_op` (D-0141), `std::proc_op` (D-0143), `std::net_op`
        // (D-0144): one shape, `(op, h, a, an, b, bn, out, cap) : i64` --
        // two byte strings in, a number back and up to `cap` bytes of an
        // answer into the arena at `out`. Each may block (a copy, a child,
        // a socket), so the GIL is released meanwhile and other threads run.
        let byte_call: Option<crate::fileio::ByteCall> = if is_std("std::path_op") {
            Some(crate::fileio::path_call)
        } else if is_std("std::proc_op") {
            Some(crate::procio::call)
        } else if is_std("std::net_op") {
            Some(crate::netio::call)
        } else {
            None
        };
        let net = is_std("std::net_op");
        if let Some(f) = byte_call {
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let addr = |v: Option<&Value>| match v {
                Some(Value::Rawptr(a, _)) => *a,
                _ => 0,
            };
            let bytes_at = |me: &Self, a: u64, n: u64| -> Vec<u8> { (0..n).map(|j| *me.arena.get(&(a + j)).unwrap_or(&0)).collect() };
            let (op, h) = (int(vals.get(0)), int(vals.get(1)));
            let a = bytes_at(self, addr(vals.get(2)), int(vals.get(3)));
            let b = bytes_at(self, addr(vals.get(4)), int(vals.get(5)));
            let (oa, cap) = (addr(vals.get(6)), int(vals.get(7)));
            self.extern_calls.push((ext.name.clone(), vec![op as i128, h as i128]));
            let (r, out) = self.unblocked(move || f(op, h, &a, &b, cap));
            // D-0204: a thread asked to stop leaves its socket's wait.
            if net && r == crate::netio::CANCELLED {
                return Err(Flow::Fault("diag.thread-cancelled".into()));
            }
            for (j, x) in out.iter().take(cap as usize).enumerate() {
                self.arena.insert(oa + j as u64, *x);
            }
            return Ok(EvalResult::Val(Value::Int(IntTy::I64, r as i128)));
        }
        // `std::os_random` (D-0142): `n` bytes from the operating system's
        // secure source into the arena; 0, or -1 when it has none.
        if is_std("std::os_random") {
            let (oa, n) = match (vals.get(0), vals.get(1)) {
                (Some(Value::Rawptr(a, _)), Some(Value::Int(_, n))) => (*a, (*n).max(0) as usize),
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            };
            self.extern_calls.push((ext.name.clone(), vec![oa as i128, n as i128]));
            let mut buf = vec![0u8; n];
            let r = crate::osrand::fill(&mut buf);
            if r == 0 {
                for (j, x) in buf.iter().enumerate() {
                    self.arena.insert(oa + j as u64, *x);
                }
            }
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, r as i128)));
        }
        // `std::fs_op`, `std::fs_query`, `std::clock_read` (D-0061): the
        // filesystem, the environment and the clocks (`fileio`), bytes to
        // and from the arena.
        // D-0124: the thread sleeps with the GIL released, so others run.
        if is_std("std::sleep_ns") {
            let ns = match vals.get(0) { Some(Value::Int(_, x)) => *x as u64, _ => 0 };
            self.extern_calls.push((ext.name.clone(), vec![ns as i128]));
            if self.gil.as_ref().and_then(|w| w.upgrade()).is_some() {
                // Waits as `join` and `lock` do: the GIL released in slices,
                // other threads running, this thread's frames restored after.
                let deadline = crate::fileio::clock_read(0).saturating_add(ns as i64);
                self.wait_for(Wait::Timer, |_| crate::fileio::clock_read(0) >= deadline)?;
            } else {
                crate::fileio::sleep_ns(ns);
            }
            return Ok(EvalResult::Val(Value::Int(IntTy::I64, 0)));
        }
        // `std::tz_offset` (D-0147): the machine's offset from UTC at a moment.
        if is_std("std::tz_offset") {
            let unix = match vals.get(0) {
                Some(Value::Int(_, x)) => *x as i64,
                _ => 0,
            };
            self.extern_calls.push((ext.name.clone(), vec![unix as i128]));
            return Ok(EvalResult::Val(Value::Int(IntTy::I64, crate::fileio::tz_offset(unix) as i128)));
        }
        if is_std("std::fs_op") || is_std("std::fs_query") || is_std("std::clock_read") {
            let int = |v: Option<&Value>| match v {
                Some(Value::Int(_, x)) => *x as u64,
                _ => 0,
            };
            let addr = |v: Option<&Value>| match v {
                Some(Value::Rawptr(a, _)) => *a,
                _ => 0,
            };
            let bytes_at = |me: &Self, a: u64, n: u64| -> Vec<u8> { (0..n).map(|j| *me.arena.get(&(a + j)).unwrap_or(&0)).collect() };
            let op = int(vals.get(0));
            self.extern_calls.push((ext.name.clone(), vec![op as i128]));
            if is_std("std::clock_read") {
                return Ok(EvalResult::Val(Value::Int(IntTy::I64, crate::fileio::clock_read(op) as i128)));
            }
            let a = bytes_at(self, addr(vals.get(1)), int(vals.get(2)));
            let (ba, bn) = (addr(vals.get(3)), int(vals.get(4)));
            let r: i128 = if is_std("std::fs_op") {
                let b = bytes_at(self, ba, bn);
                crate::fileio::fs_op(op, &a, &b) as i128
            } else {
                match crate::fileio::fs_query(op, &a) {
                    Ok(out) => {
                        for (j, x) in out.iter().take(bn as usize).enumerate() {
                            self.arena.insert(ba + j as u64, *x);
                        }
                        out.len() as i128
                    }
                    Err(c) => c as i128,
                }
            };
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, r)));
        }
        // `std::stdin_read` (`rule.stdlib.read`, D-0050): standard input's next
        // bytes into the arena at `buf` -- at most `len`, 0 at its end, -1
        // on an error -- through Rust's buffered, portable `stdin`, as
        // `cbrt`'s `cb_read_in` does. Standard output's buffer is written
        // out first, so a prompt appears before the wait. The GIL is released while the
        // read blocks, so other threads run on (nothing of the interpreter
        // is touched meanwhile). A program's own `extern fn stdin_read`
        // is not this one.
        let line_read = self.items.externs.get("std::stdin_read_line").map_or(false, |e| Arc::ptr_eq(e, ext));
        if line_read || self.items.externs.get("std::stdin_read").map_or(false, |e| Arc::ptr_eq(e, ext)) {
            let (addr, len) = match (vals.get(0), vals.get(1)) {
                (Some(Value::Rawptr(a, _)), Some(Value::Int(_, n))) => (*a, (*n).max(0) as usize),
                _ => return Err(Flow::Fault("diag.type-mismatch".into())),
            };
            let mut buf = vec![0u8; len];
            let gil = if self.active_threads > 1 { self.gil.as_ref().and_then(|w| w.upgrade()) } else { None };
            let me = self.current_thread;
            if let Some(g) = &gil {
                g.io(me, true);
                g.release();
            }
            // `std::stdin_read_line` (`read_line`'s primitive): through the
            // next `\n` (src/fileio.rs, as `cbrt`'s `cb_read_line_in`).
            let got = if line_read {
                match crate::fileio::read_line_chunk(&mut buf) {
                    k if k < 0 => Err(std::io::Error::from(std::io::ErrorKind::Other)),
                    k => Ok(k as usize),
                }
            } else {
                use std::io::Read;
                let _ = crate::outbuf::flush();
                let mut input = std::io::stdin().lock();
                loop {
                    match input.read(&mut buf) {
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        other => break other,
                    }
                }
            };
            if let Some(g) = &gil {
                g.io(me, false);
                g.acquire();
                std::hint::black_box(&mut *self);
                self.current_thread = me;
            }
            let n: i128 = match got {
                Ok(n) => {
                    for (j, b) in buf[..n].iter().enumerate() {
                        self.arena.insert(addr + j as u64, *b);
                    }
                    n as i128
                }
                Err(_) => -1,
            };
            self.extern_calls.push((ext.name.clone(), vec![addr as i128, len as i128]));
            return Ok(EvalResult::Val(Value::Int(IntTy::Isize, n)));
        }
        let to_err = self.items.externs.get("std::stderr_write").map_or(false, |e| Arc::ptr_eq(e, ext));
        let to_out = self.items.externs.get("std::stdout_write").map_or(false, |e| Arc::ptr_eq(e, ext));
        if to_out || to_err {
            if let (Some(Value::Rawptr(addr, _)), Some(Value::Int(_, len))) = (vals.get(0), vals.get(1)) {
                let mut buf = Vec::with_capacity(*len as usize);
                for i in 0..*len {
                    buf.push(*self.arena.get(&(addr + i as u64)).unwrap_or(&0));
                }
                // Bytes, not text: no newline translation on any platform.
                // Standard output is buffered, standard error not (D-0178).
                let res = if to_err { crate::outbuf::write_err(&buf) } else { crate::outbuf::write(&buf) };
                let n: i128 = match res {
                    Ok(()) => buf.len() as i128,
                    Err(_) => -1,
                };
                self.extern_calls.push((ext.name.clone(), vec![*addr as i128, *len]));
                return Ok(EvalResult::Val(Value::Int(IntTy::Isize, n)));
            }
        }
        // `[Extern-Call]`: a real call into C (`ffi.rs`); the result is a
        // value of the declared type, whose meaning is an unchecked claim.
        self.extern_calls.push((ext.name.clone(), vec![]));
        let ptys: Vec<Type> = ext.params.iter().map(|p| self.apply_subst(&p.ty, subst)).collect();
        match crate::ffi::call(&ext.name, &ptys, &vals, &ext.ret) {
            Ok(v) => Ok(EvalResult::Val(v)),
            Err(crate::ffi::Refusal::Unsupported(msg)) => {
                // The program's own C or assembly code (`extern "…";`) is
                // `cobc`'s to link; name it, so the message says where the
                // function is.
                let msg = if msg.starts_with("no C function") && !self.items.extern_code.is_empty() {
                    let code: Vec<String> = self.items.extern_code.iter().map(|(c, _)| format!("\"{}\"", c)).collect();
                    format!("{}; the program declares foreign code ({}), which only cobc links", msg, code.join(", "))
                } else {
                    msg
                };
                Err(Flow::Fault(format!("unsupported: {}", msg)))
            }
        }
    }

    // ---- unsafe / rawptr / arena ----

    pub(super) fn arena_read(&self, addr: u64, ty: &Type) -> Value {
        let sz = self.sizeof_ty(ty);
        let mut bytes = Vec::with_capacity(sz as usize);
        for i in 0..sz {
            bytes.push(*self.arena.get(&(addr + i)).unwrap_or(&0));
        }
        let v = self.decode(ty, &bytes);
        if self.arena_side.range(addr..addr + sz.max(1)).next().is_none() {
            return v;
        }
        self.patch_side(ty, v, addr)
    }
    pub(super) fn arena_write(&mut self, addr: u64, ty: &Type, v: Value) {
        self.intern_strs_in(&v);
        let mut side = Vec::new();
        self.side_values(ty, &v, 0, &mut side);
        // A reference's bytes are its referent's address (`[Repr-Ref]`),
        // so the referent needs one before the value is encoded.
        for (_, sv) in &side {
            if let Value::Ref { obj, .. } = sv {
                self.object_addr(*obj);
            }
        }
        let bytes = self.encode(ty, &v);
        let sz = bytes.len() as u64;
        for (i, b) in bytes.into_iter().enumerate() {
            self.arena.insert(addr + i as u64, b);
        }
        self.clear_side(addr, sz);
        for (off, sv) in side {
            self.arena_side.insert(addr + off, sv);
        }
    }

    pub(super) fn clear_side(&mut self, addr: u64, n: u64) {
        let gone: Vec<u64> = self.arena_side.range(addr..addr + n.max(1)).map(|(a, _)| *a).collect();
        for a in gone {
            self.arena_side.remove(&a);
        }
    }

    // The values inside `v : ty` that bytes cannot carry, by offset.
    pub(super) fn side_values(&self, ty: &Type, v: &Value, off: u64, out: &mut Vec<(u64, Value)>) {
        match (ty, v) {
            (Type::Ref(..), Value::Ref { .. }) | (Type::Guard(_), Value::Guard { .. }) | (Type::Handle(_), Value::Handle(..)) | (Type::Fn(..), Value::FnVal(_)) | (Type::Fn(..), Value::Closure(_)) => {
                out.push((off, v.clone()));
            }
            (Type::Named(name, args), Value::Struct(fields)) => {
                if let (Some(s), Some(layout)) = (self.items.structs.get(name), self.struct_layout(name, args)) {
                    let subst = self.subst_map(&s.type_params, args);
                    for (i, f) in fields.iter().enumerate() {
                        let fty = self.apply_subst(&s.fields[i].ty, &subst);
                        self.side_values(&fty, f, off + layout.offsets[i], out);
                    }
                }
            }
            (Type::Named(name, args), Value::Enum(vi, payload)) => {
                if let Some(layout) = self.enum_layout(name, args) {
                    if let Some(pty) = layout.payload_types.get(*vi) {
                        self.side_values(pty, payload, off + layout.payload_off, out);
                    }
                }
            }
            (Type::Mutex(inner), Value::Struct(fields)) => {
                if let Some(f) = fields.first() {
                    self.side_values(inner, f, off, out);
                }
            }
            // A slice (D-0047): its reference at offset 0, then two words.
            (Type::Slice(inner, m), Value::Struct(fields)) => {
                if let Some(f) = fields.first() {
                    self.side_values(&Type::Ref(inner.clone(), m.clone()), f, off, out);
                }
            }
            (Type::Array(inner, _) | Type::Simd(inner, _), Value::Array(items)) => {
                let es = self.sizeof_ty(inner);
                for (i, it) in items.iter().enumerate() {
                    self.side_values(inner, it, off + i as u64 * es, out);
                }
            }
            _ => {}
        }
    }

    // `v`, decoded from the bytes at `addr`, with the side values put back.
    pub(super) fn patch_side(&self, ty: &Type, v: Value, addr: u64) -> Value {
        match (ty, v) {
            (Type::Ref(..), v) | (Type::Guard(_), v) | (Type::Handle(_), v) | (Type::Fn(..), v) => self.arena_side.get(&addr).cloned().unwrap_or(v),
            (Type::Named(name, args), Value::Struct(fields)) => match (self.items.structs.get(name), self.struct_layout(name, args)) {
                (Some(s), Some(layout)) => {
                    let subst = self.subst_map(&s.type_params, args);
                    let fields = fields
                        .into_iter()
                        .enumerate()
                        .map(|(i, f)| {
                            let fty = self.apply_subst(&s.fields[i].ty, &subst);
                            self.patch_side(&fty, f, addr + layout.offsets[i])
                        })
                        .collect();
                    Value::Struct(fields)
                }
                _ => Value::Struct(fields),
            },
            (Type::Named(name, args), Value::Enum(vi, payload)) => match self.enum_layout(name, args) {
                Some(layout) => match layout.payload_types.get(vi) {
                    Some(pty) => Value::Enum(vi, Box::new(self.patch_side(pty, *payload, addr + layout.payload_off))),
                    None => Value::Enum(vi, payload),
                },
                None => Value::Enum(vi, payload),
            },
            (Type::Mutex(inner), Value::Struct(mut fields)) => {
                if !fields.is_empty() {
                    let f = fields.remove(0);
                    fields.insert(0, self.patch_side(inner, f, addr));
                }
                Value::Struct(fields)
            }
            (Type::Slice(inner, m), Value::Struct(mut fields)) => {
                if !fields.is_empty() {
                    let f = fields.remove(0);
                    fields.insert(0, self.patch_side(&Type::Ref(inner.clone(), m.clone()), f, addr));
                }
                Value::Struct(fields)
            }
            (Type::Array(inner, _) | Type::Simd(inner, _), Value::Array(items)) => {
                let es = self.sizeof_ty(inner);
                Value::Array(items.into_iter().enumerate().map(|(i, it)| self.patch_side(inner, it, addr + i as u64 * es)).collect())
            }
            (_, v) => v,
        }
    }

    pub(super) fn encode(&self, ty: &Type, v: &Value) -> Vec<u8> {
        match (ty, v) {
            (Type::Int(t), Value::Int(_, x)) => {
                let bw = t.bitwidth(ADDR_WIDTH);
                (*x as u128).to_le_bytes()[..(bw / 8) as usize].to_vec()
            }
            (Type::Bool, Value::Bool(b)) => vec![if *b { 1 } else { 0 }],
            (Type::F64, Value::F64(f)) => f.to_le_bytes().to_vec(),
            (Type::F32, Value::F32(f)) => f.to_le_bytes().to_vec(),
            (Type::Rawptr(_), Value::Rawptr(a, _)) => a.to_le_bytes().to_vec(),
            (Type::Str, Value::Str(s)) => {
                // [Repr-Str] (impl-defined): (address of program data, length).
                let addr = self.str_data.get(s).copied().unwrap_or(0);
                let mut out = addr.to_le_bytes().to_vec();
                out.extend_from_slice(&(s.len() as u64).to_le_bytes());
                out
            }
            // [Repr-Ref]: the image of the referent's address, as a rawptr
            // to the same cells shows (the reference value itself is kept
            // aside, `side_values`).
            (Type::Ref(..), Value::Ref { obj, path, .. }) => self.ref_image(*obj, path),
            (Type::Ref(..), _) | (Type::Handle(_), _) | (Type::Guard(_), _) | (Type::Fn(..), Value::Closure(_)) => {
                // stored as an opaque object-id token in our arena model
                // (a closure's callable box too: its value is kept aside)
                0u64.to_le_bytes().to_vec()
            }
            (Type::Fn(..), Value::FnVal(name)) => {
                // [Repr-Fn] (spec/06 sec 4): "any injective AddrWidth/8-
                // byte image of an item path" -- a deterministic hash of
                // the qualified name, not a real reversible encoding
                // (decode does not attempt to reconstruct a callable
                // FnVal from this; see the "known gaps" note in
                // STATUS.md -- the spec's own text says this
                // representation is "only consumed by ==/!= on fn
                // values and by spawn's callee argument", both of which
                // this interpreter already handles directly on the
                // Value::FnVal representation, never via an arena
                // round-trip).
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut h = DefaultHasher::new();
                name.hash(&mut h);
                h.finish().to_le_bytes().to_vec()
            }
            (Type::Named(name, args), Value::Struct(fields)) => {
                let mut out = vec![0u8; self.sizeof_ty(ty) as usize];
                // D-0118: `[Repr-Bitstruct]`: the backing integer's image.
                if let Some((b, widths)) = self.bitstruct_decl(name) {
                    let n = (b.bitwidth(ADDR_WIDTH) / 8) as usize;
                    return Self::pack_bits(&widths, fields).to_le_bytes()[..n].to_vec();
                }
                if let Some(s) = self.items.structs.get(name) {
                    let subst = self.subst_map(&s.type_params, args);
                    if let Some(layout) = self.struct_layout(name, args) {
                        for (i, f) in fields.iter().enumerate() {
                            let fty = self.apply_subst(&s.fields[i].ty, &subst);
                            let fb = self.encode(&fty, f);
                            let off = layout.offsets[i] as usize;
                            out[off..off + fb.len()].copy_from_slice(&fb);
                        }
                    }
                }
                out
            }
            (Type::Named(name, args), Value::Enum(vi, payload)) => {
                // [Repr-Enum] (spec/06 §7): the DW-byte image of the
                // variant index at offset 0, the payload's own
                // represent() at payload-off, pad elsewhere.
                let mut out = vec![0u8; self.sizeof_ty(ty) as usize];
                if let Some(layout) = self.enum_layout(name, args) {
                    let disc = (*vi as u64).to_le_bytes();
                    let dw = layout.dw as usize;
                    out[0..dw].copy_from_slice(&disc[..dw]);
                    if let Some(pty) = layout.payload_types.get(*vi) {
                        let pb = self.encode(pty, payload);
                        let off = layout.payload_off as usize;
                        out[off..off + pb.len()].copy_from_slice(&pb);
                    }
                }
                out
            }
            (Type::Mutex(inner), Value::Struct(fields)) => {
                // [Repr-Mutex] (spec/06 sec 4): represent(inner,v) at
                // off(inner)=0; the state cells (bytes sizeof(inner)..
                // sizeof(inner)+8) are outcome: impl-defined/never read
                // by a rule -- left zeroed, a valid instance of that
                // freedom, not a placeholder standing in for a real value.
                let mut out = vec![0u8; self.sizeof_ty(ty) as usize];
                if let Some(v) = fields.first() {
                    let ib = self.encode(inner, v);
                    out[0..ib.len()].copy_from_slice(&ib);
                }
                out
            }
            (Type::Slice(..), Value::Struct(fields)) => {
                // The reference's word (kept beside, as any reference's),
                // then the start and the length.
                let mut out = vec![0u8; 8];
                for f in fields.iter().skip(1) {
                    let x = if let Value::Int(_, x) = f { *x as u64 } else { 0 };
                    out.extend_from_slice(&x.to_le_bytes());
                }
                out.resize(24, 0);
                out
            }
            (Type::Array(inner, _) | Type::Simd(inner, _), Value::Array(items)) => {
                // [Repr-Array] (spec/06 §7): represent(τ,vi) placed at i·sizeof(τ).
                let elem_sz = self.sizeof_ty(inner) as usize;
                let mut out = Vec::with_capacity(elem_sz * items.len());
                for item in items {
                    let mut eb = self.encode(inner, item);
                    eb.resize(elem_sz, 0);
                    out.extend_from_slice(&eb);
                }
                out
            }
            _ => vec![0u8; self.sizeof_ty(ty) as usize],
        }
    }

    pub(super) fn decode(&self, ty: &Type, bytes: &[u8]) -> Value {
        match ty {
            Type::Int(t) => {
                let bw = (t.bitwidth(ADDR_WIDTH) / 8) as usize;
                let mut buf = [0u8; 16];
                buf[..bw.min(16)].copy_from_slice(&bytes[..bw.min(16)]);
                let u = u128::from_le_bytes(buf);
                Value::Int(*t, reinterpret_sign_pub(*t, u, t.bitwidth(ADDR_WIDTH)))
            }
            Type::Bool => Value::Bool(bytes.first().copied().unwrap_or(0) != 0),
            Type::F64 => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&bytes[..8]);
                Value::F64(f64::from_le_bytes(b))
            }
            Type::F32 => {
                let mut b = [0u8; 4];
                b.copy_from_slice(&bytes[..4]);
                Value::F32(f32::from_le_bytes(b))
            }
            Type::Rawptr(inner) => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&bytes[..8]);
                Value::Rawptr(u64::from_le_bytes(b), (**inner).clone())
            }
            Type::Str => {
                let mut a = [0u8; 8];
                let mut l = [0u8; 8];
                a.copy_from_slice(&bytes[..8]);
                l.copy_from_slice(&bytes[8..16]);
                let (addr, len) = (u64::from_le_bytes(a), u64::from_le_bytes(l));
                let data: Vec<u8> = (0..len).map(|i| *self.arena.get(&(addr + i)).unwrap_or(&0)).collect();
                Value::Str(Arc::from(data))
            }
            Type::Named(name, args) => {
                if let Some((b, _)) = self.bitstruct_decl(name) {
                    let n = (b.bitwidth(ADDR_WIDTH) / 8) as usize;
                    let mut buf = [0u8; 16];
                    buf[..n.min(16)].copy_from_slice(&bytes[..n.min(16)]);
                    return self.unpack_bits(name, u128::from_le_bytes(buf));
                }
                if let Some(s) = self.items.structs.get(name).cloned() {
                    let subst = self.subst_map(&s.type_params, args);
                    if let Some(layout) = self.struct_layout(name, args) {
                        let mut fields = Vec::new();
                        for (i, f) in s.fields.iter().enumerate() {
                            let fty = self.apply_subst(&f.ty, &subst);
                            let sz = self.sizeof_ty(&fty) as usize;
                            let off = layout.offsets[i] as usize;
                            fields.push(self.decode(&fty, &bytes[off..off + sz]));
                        }
                        return Value::Struct(fields);
                    }
                }
                if let Some(layout) = self.enum_layout(name, args) {
                    let dw = layout.dw as usize;
                    let mut buf = [0u8; 8];
                    buf[..dw.min(8)].copy_from_slice(&bytes[..dw.min(8)]);
                    let vi = u64::from_le_bytes(buf) as usize;
                    let pty = layout.payload_types.get(vi).cloned().unwrap_or(Type::Void);
                    let off = layout.payload_off as usize;
                    let psz = self.sizeof_ty(&pty) as usize;
                    let payload = if off + psz <= bytes.len() {
                        self.decode(&pty, &bytes[off..off + psz])
                    } else {
                        Value::Unit
                    };
                    return Value::Enum(vi, Box::new(payload));
                }
                Value::Unit
            }
            Type::Mutex(inner) => {
                let isz = self.sizeof_ty(inner) as usize;
                let v = if isz <= bytes.len() {
                    self.decode(inner, &bytes[..isz])
                } else {
                    Value::Unit
                };
                Value::Struct(vec![v])
            }
            Type::Array(inner, n) | Type::Simd(inner, n) => {
                let elem_sz = self.sizeof_ty(inner) as usize;
                let mut items = Vec::with_capacity(*n as usize);
                for i in 0..*n as usize {
                    let off = i * elem_sz;
                    if off + elem_sz > bytes.len() {
                        break;
                    }
                    items.push(self.decode(inner, &bytes[off..off + elem_sz]));
                }
                Value::Array(items)
            }
            // A slice (D-0047): the reference (put back by `patch_side`),
            // the start, the length.
            Type::Slice(..) => {
                let word = |k: usize| -> i128 {
                    let mut b = [0u8; 8];
                    if bytes.len() >= k + 8 {
                        b.copy_from_slice(&bytes[k..k + 8]);
                    }
                    u64::from_le_bytes(b) as i128
                };
                Value::Struct(vec![Value::Unit, Value::Int(IntTy::Usize, word(8)), Value::Int(IntTy::Usize, word(16))])
            }
            _ => Value::Unit,
        }
    }
}
