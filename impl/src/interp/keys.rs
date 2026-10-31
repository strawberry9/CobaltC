// Key encodings and comparisons, and the bytes of strings.

use super::*;

impl Interp {
    // The bytes of the `String` at `path` in `obj`: its `Vec<u8>`'s
    // `len` bytes at `ptr`, in the arena.
    pub(super) fn string_bytes(&self, obj: u64, path: &[Proj]) -> Vec<u8> {
        let Value::Struct(fields) = self.read_place(obj, path) else { return Vec::new() };
        let Some(Value::Struct(vec)) = fields.first() else { return Vec::new() };
        let (Some(Value::Rawptr(addr, _)), Some(Value::Int(_, len))) = (vec.first(), vec.get(1)) else { return Vec::new() };
        (0..*len as u64).map(|j| *self.arena.get(&(addr + j)).unwrap_or(&0)).collect()
    }

    // A key's bytes (`rule.stdlib.hashmap`): an integer's little-endian
    // bytes in its own width, a `bool`'s one byte, text's UTF-8 bytes.
    pub(super) fn key_bytes(&self, v: Value) -> Result<Vec<u8>, Flow> {
        let Value::Ref { obj, path, pointee, .. } = v else {
            return Err(Flow::Fault("diag.type-mismatch".into()));
        };
        if !self.object_live(obj) {
            return Err(Flow::Fault("diag.stale-binding".into()));
        }
        if matches!(&pointee, Type::Named(n, _) if n == "std::String") {
            return Ok(self.string_bytes(obj, &path));
        }
        match self.read_place(obj, &path) {
            Value::Int(t, x) => Ok((x as u128).to_le_bytes()[..(t.bitwidth(ADDR_WIDTH) / 8) as usize].to_vec()),
            Value::Bool(b) => Ok(vec![b as u8]),
            Value::Str(b) => Ok(b.to_vec()),
            v @ (Value::Struct(_) | Value::Enum(..)) => {
                let mut out = Vec::new();
                self.key_enc(&v, &pointee, &mut out)?;
                Ok(out)
            }
            _ => Err(Flow::Fault("diag.type-mismatch".into())),
        }
    }

    // D-0110: a struct or enum key's bytes (`[Key-Bytes]`): its fields'
    // in declaration order, an enum's variant index (a `u32`) then its
    // payload's; text inside one is its length (a `u64`) then its bytes.
    pub(super) fn key_enc(&self, v: &Value, ty: &Type, out: &mut Vec<u8>) -> Result<(), Flow> {
        match (ty, v) {
            (_, Value::Int(t, x)) => out.extend_from_slice(&(*x as u128).to_le_bytes()[..(t.bitwidth(ADDR_WIDTH) / 8) as usize]),
            (_, Value::Bool(b)) => out.push(*b as u8),
            (_, Value::Str(b)) => {
                out.extend_from_slice(&(b.len() as u64).to_le_bytes());
                out.extend_from_slice(b);
            }
            (Type::Named(n, _), _) if n == "std::String" => {
                let b = self.string_value_bytes(v);
                out.extend_from_slice(&(b.len() as u64).to_le_bytes());
                out.extend_from_slice(&b);
            }
            // D-0118: a bitstruct key is its backing integer's bytes.
            (Type::Named(n, _), Value::Struct(fs)) if self.bitstruct_decl(n).is_some() => {
                let (b, widths) = self.bitstruct_decl(n).unwrap();
                let k = (b.bitwidth(ADDR_WIDTH) / 8) as usize;
                out.extend_from_slice(&Self::pack_bits(&widths, fs).to_le_bytes()[..k]);
            }
            (Type::Named(n, args), Value::Struct(fs)) if self.items.structs.contains_key(n) => {
                let sd = self.items.structs.get(n).unwrap().clone();
                let sub = self.subst_map(&sd.type_params, args);
                for (f, fv) in sd.fields.iter().zip(fs.iter()) {
                    self.key_enc(fv, &self.apply_subst(&f.ty, &sub), out)?;
                }
            }
            (Type::Named(n, args), Value::Enum(vi, p)) if self.items.enums.contains_key(n) => {
                let ed = self.items.enums.get(n).unwrap().clone();
                let sub = self.subst_map(&ed.type_params, args);
                out.extend_from_slice(&(*vi as u32).to_le_bytes());
                if let Some(pt) = ed.variants.get(*vi).and_then(|x| x.payload.as_ref()) {
                    self.key_enc(p, &self.apply_subst(pt, &sub), out)?;
                }
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        }
        Ok(())
    }

    // D-0110: the key types' order on two values of type `ty`: integers by
    // value, `false` first, text by its bytes; a struct field by field in
    // declaration order, an enum by variant order, then by payload.
    pub(super) fn key_cmp(&self, a: &Value, b: &Value, ty: &Type) -> Result<std::cmp::Ordering, Flow> {
        Ok(match (ty, a, b) {
            (_, Value::Int(t, x), Value::Int(_, y)) => {
                if t.signed() {
                    x.cmp(y)
                } else {
                    (*x as u128).cmp(&(*y as u128))
                }
            }
            (_, Value::Bool(x), Value::Bool(y)) => x.cmp(y),
            (_, Value::Str(x), Value::Str(y)) => x.cmp(y),
            (Type::Named(n, _), _, _) if n == "std::String" => self.string_value_bytes(a).cmp(&self.string_value_bytes(b)),
            (Type::Named(n, args), Value::Struct(fa), Value::Struct(fb)) if self.items.structs.contains_key(n) => {
                let sd = self.items.structs.get(n).unwrap().clone();
                let sub = self.subst_map(&sd.type_params, args);
                for ((f, x), y) in sd.fields.iter().zip(fa.iter()).zip(fb.iter()) {
                    let o = self.key_cmp(x, y, &self.apply_subst(&f.ty, &sub))?;
                    if o.is_ne() {
                        return Ok(o);
                    }
                }
                std::cmp::Ordering::Equal
            }
            (Type::Named(n, args), Value::Enum(va, pa), Value::Enum(vb, pb)) if self.items.enums.contains_key(n) => {
                if va != vb {
                    return Ok(va.cmp(vb));
                }
                let ed = self.items.enums.get(n).unwrap().clone();
                let sub = self.subst_map(&ed.type_params, args);
                match ed.variants.get(*va).and_then(|x| x.payload.as_ref()) {
                    Some(pt) => self.key_cmp(pa, pb, &self.apply_subst(pt, &sub))?,
                    None => std::cmp::Ordering::Equal,
                }
            }
            _ => return Err(Flow::Fault("diag.type-mismatch".into())),
        })
    }

    // A `String` value's bytes, in the arena.
    // The bytes of a text value of type `ty` (D-0127): `str`, `String` or
    // `StringView`; `None` for any other type.
    pub(super) fn text_bytes_of(&self, v: &Value, ty: &Type) -> Option<Vec<u8>> {
        match (ty, v) {
            (Type::Str, Value::Str(b)) => Some(b.to_vec()),
            (Type::Named(n, _), Value::Struct(_)) if n == "std::String" => Some(self.string_value_bytes(v)),
            (Type::Named(n, _), Value::Struct(_)) if n == "std::StringView" => self.view_bytes(v).ok(),
            _ => None,
        }
    }

    pub(super) fn string_value_bytes(&self, v: &Value) -> Vec<u8> {
        let Value::Struct(fields) = v else { return Vec::new() };
        let Some(Value::Struct(vec)) = fields.first() else { return Vec::new() };
        let (Some(Value::Rawptr(addr, _)), Some(Value::Int(_, len))) = (vec.first(), vec.get(1)) else { return Vec::new() };
        (0..*len as u64).map(|j| *self.arena.get(&(addr + j)).unwrap_or(&0)).collect()
    }
}
