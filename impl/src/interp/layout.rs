// bitstructs, and struct and enum layouts.

use super::*;

impl Interp {
    // D-0118: a `bitstruct`'s backing type and field widths, low bit first.
    pub(super) fn bitstruct_decl(&self, name: &str) -> Option<(IntTy, Vec<u32>)> {
        let s = self.items.structs.get(name)?;
        let b = s.bits?;
        Some((b, s.fields.iter().map(|f| f.width.unwrap_or(0)).collect()))
    }
    pub(super) fn bits_mask(w: u32) -> u128 {
        if w >= 128 { u128::MAX } else { (1u128 << w) - 1 }
    }
    // The backing integer of a bitstruct value (`Name::bits`).
    pub(super) fn pack_bits(widths: &[u32], fields: &[Value]) -> u128 {
        let mut acc = 0u128;
        let mut off = 0u32;
        for (w, v) in widths.iter().zip(fields.iter()) {
            if let Value::Int(_, x) = v {
                acc |= ((*x as u128) & Self::bits_mask(*w)) << off;
            }
            off += w;
        }
        acc
    }
    // The fields of a bitstruct value from its backing integer (`Name::from_bits`).
    pub(super) fn unpack_bits(&self, name: &str, bits: u128) -> Value {
        let s = self.items.structs.get(name).expect("bitstruct");
        let mut out = Vec::with_capacity(s.fields.len());
        let mut off = 0u32;
        for f in &s.fields {
            let w = f.width.unwrap_or(0);
            let t = match f.ty { Type::Int(t) => t, _ => IntTy::U8 };
            out.push(Value::Int(t, ((bits >> off) & Self::bits_mask(w)) as i128));
            off += w;
        }
        Value::Struct(out)
    }
    // Where a path into `ty` enters a bitstruct's field: the prefix length.
    pub(super) fn bitstruct_split(&self, base: u64, ty: &Type, path: &[Proj]) -> Option<usize> {
        for k in 0..path.len() {
            if !matches!(path[k], Proj::Field(_)) {
                continue;
            }
            let (_, t) = self.path_offset_at(Some(base), ty, &path[..k]);
            if let Type::Named(n, _) = &t {
                if self.bitstruct_decl(n).is_some() {
                    return Some(k);
                }
            }
        }
        None
    }

    pub(super) fn struct_layout(&self, name: &str, args: &[Type]) -> Option<Layout> {
        let key = (name.to_string(), args.to_vec());
        if let Some(l) = self.struct_layouts.borrow().get(&key) {
            return l.clone();
        }
        let l = self.struct_layout_uncached(name, args);
        self.struct_layouts.borrow_mut().insert(key, l.clone());
        l
    }

    pub(super) fn struct_layout_uncached(&self, name: &str, args: &[Type]) -> Option<Layout> {
        let s = self.items.structs.get(name)?;
        // D-0118: a bitstruct is its backing integer; its fields have no
        // offsets of their own.
        if let Some(b) = s.bits {
            let n = (b.bitwidth(ADDR_WIDTH) / 8) as u64;
            return Some(Layout { size: n, align: n.min(8), offsets: vec![0; s.fields.len()] });
        }
        let subst = self.subst_map(&s.type_params, args);
        let mut off = 0u64;
        // D-0251: `align(N)` raises the struct's alignment, never lowers it.
        let mut align = s.align.unwrap_or(1);
        let mut offsets = Vec::new();
        for f in &s.fields {
            let fty = self.apply_subst(&f.ty, &subst);
            let fa = self.alignof_ty(&fty);
            align = align.max(fa);
            off = (off + fa - 1) / fa * fa;
            offsets.push(off);
            off += self.sizeof_ty(&fty);
        }
        off = (off + align - 1) / align * align;
        Some(Layout { size: off, align, offsets })
    }

    // [Layout-Enum] (spec/16 §1): discriminant at offset 0, width DW;
    // payload-off = least multiple of max(alignof(τi)) >= DW;
    // sizeof = least multiple of alignof >= payload-off + max(sizeof(τi));
    // alignof = max(DW, alignof(τi)). DW = 4 is this implementation's
    // documented impl-defined choice (impl/STATUS.md).
    pub(super) fn enum_layout(&self, name: &str, args: &[Type]) -> Option<EnumLayout> {
        let key = (name.to_string(), args.to_vec());
        if let Some(l) = self.enum_layouts.borrow().get(&key) {
            return l.clone();
        }
        let l = self.enum_layout_uncached(name, args);
        self.enum_layouts.borrow_mut().insert(key, l.clone());
        l
    }

    pub(super) fn enum_layout_uncached(&self, name: &str, args: &[Type]) -> Option<EnumLayout> {
        let dw = ENUM_DISCRIMINANT_WIDTH as u64;
        let e = self.items.enums.get(name)?.clone();
        let subst = self.subst_map(&e.type_params, args);
        let mut max_payload_align = 1u64;
        let mut max_payload_size = 0u64;
        let mut payload_types = Vec::new();
        for v in &e.variants {
            let pty = match &v.payload {
                Some(t) => self.apply_subst(t, &subst),
                None => Type::Void,
            };
            max_payload_align = max_payload_align.max(self.alignof_ty(&pty));
            max_payload_size = max_payload_size.max(self.sizeof_ty(&pty));
            payload_types.push(pty);
        }
        let payload_off = (dw + max_payload_align - 1) / max_payload_align * max_payload_align;
        let align = dw.max(max_payload_align);
        let raw = payload_off + max_payload_size;
        let size = (raw + align - 1) / align * align;
        Some(EnumLayout { size, align, dw, payload_off, payload_types })
    }
}
