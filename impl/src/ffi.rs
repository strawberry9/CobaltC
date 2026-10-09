// Real calls to C functions for `extern fn` declarations (`spec/20` §3,
// `[Extern-Call]`). The specification leaves symbol lookup and calling
// convention to the implementation; this interpreter's choices:
//
// - The symbol is the declared name, looked up in the running process
//   (libc) and, failing that, in libm, which is loaded on first use.
// - x86-64 System V only. Integer, `bool` and pointer arguments go in
//   the six integer registers and `f32`/`f64` arguments in the eight
//   vector registers, independently, so a single function-pointer type
//   with six integer and eight float parameters calls any callee using
//   at most that many of each: the callee reads only the registers its
//   own signature names. Stack-passed arguments and variadic callees
//   (which also read `%al`) are not supported.
// - `rawptr` arguments and results are refused: this interpreter's
//   memory is a simulated arena, so its addresses mean nothing to C.
//   The native compiler, `cobc`, has no such limit.

#[cfg(all(unix, target_arch = "x86_64"))]
use crate::ast::IntTy;
use crate::ast::Type;
use crate::value::Value;

pub enum Refusal {
    Unsupported(String),
}

#[cfg(all(unix, target_arch = "x86_64"))]
mod native {
    use std::ffi::{c_char, c_void, CString};
    use std::sync::OnceLock;

    extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlopen(file: *const c_char, mode: i32) -> *mut c_void;
    }
    const RTLD_NOW: i32 = 2;
    const RTLD_GLOBAL: i32 = 0x100;

    static LIBM: OnceLock<usize> = OnceLock::new();

    pub fn lookup(name: &str) -> Option<usize> {
        let sym = CString::new(name).ok()?;
        unsafe {
            let p = dlsym(std::ptr::null_mut(), sym.as_ptr());
            if !p.is_null() {
                return Some(p as usize);
            }
            let libm = *LIBM.get_or_init(|| {
                let file = CString::new("libm.so.6").unwrap();
                dlopen(file.as_ptr(), RTLD_NOW | RTLD_GLOBAL) as usize
            });
            if libm == 0 {
                return None;
            }
            let p = dlsym(libm as *mut c_void, sym.as_ptr());
            if p.is_null() {
                None
            } else {
                Some(p as usize)
            }
        }
    }

    pub enum Ret {
        Int,
        F64,
        F32,
    }

    type IntFn = unsafe extern "C" fn(u64, u64, u64, u64, u64, u64, f64, f64, f64, f64, f64, f64, f64, f64) -> u64;
    type F64Fn = unsafe extern "C" fn(u64, u64, u64, u64, u64, u64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64;
    type F32Fn = unsafe extern "C" fn(u64, u64, u64, u64, u64, u64, f64, f64, f64, f64, f64, f64, f64, f64) -> f32;

    // The raw result: the integer register, or the float's bits.
    pub unsafe fn call(f: usize, i: &[u64; 6], x: &[f64; 8], ret: Ret) -> u64 {
        match ret {
            Ret::Int => {
                let f: IntFn = std::mem::transmute(f);
                f(i[0], i[1], i[2], i[3], i[4], i[5], x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7])
            }
            Ret::F64 => {
                let f: F64Fn = std::mem::transmute(f);
                f(i[0], i[1], i[2], i[3], i[4], i[5], x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]).to_bits()
            }
            Ret::F32 => {
                let f: F32Fn = std::mem::transmute(f);
                f(i[0], i[1], i[2], i[3], i[4], i[5], x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]).to_bits() as u64
            }
        }
    }
}

fn refuse(msg: String) -> Refusal {
    Refusal::Unsupported(msg)
}

/// Calls the C function `name` with already-evaluated arguments of the
/// declared FfiTypes, returning a value of type `ret`.
#[cfg(all(unix, target_arch = "x86_64"))]
pub fn call(name: &str, params: &[Type], args: &[Value], ret: &Type) -> Result<Value, Refusal> {
    let mut ints = [0u64; 6];
    let mut floats = [0f64; 8];
    let (mut ni, mut nf) = (0usize, 0usize);
    let mut push_int = |v: u64, ni: &mut usize| -> Result<(), Refusal> {
        if *ni == 6 {
            return Err(refuse(format!("extern fn `{}` takes more than six integer arguments", name)));
        }
        ints[*ni] = v;
        *ni += 1;
        Ok(())
    };
    for (ty, v) in params.iter().zip(args.iter()) {
        match (ty, v) {
            (Type::Int(t), Value::Int(_, x)) => {
                if matches!(t, IntTy::I128 | IntTy::U128) {
                    push_int(*x as u128 as u64, &mut ni)?;
                    push_int((*x as u128 >> 64) as u64, &mut ni)?;
                } else {
                    // Signed values sign-extend, unsigned ones zero-extend.
                    push_int(*x as i64 as u64, &mut ni)?;
                }
            }
            (Type::Bool, Value::Bool(b)) => push_int(*b as u64, &mut ni)?,
            (Type::F64, Value::F64(f)) | (Type::F32, Value::F64(f)) => {
                if nf == 8 {
                    return Err(refuse(format!("extern fn `{}` takes more than eight float arguments", name)));
                }
                floats[nf] = if matches!(ty, Type::F32) { f64::from_bits((*f as f32).to_bits() as u64) } else { *f };
                nf += 1;
            }
            (Type::F32, Value::F32(f)) | (Type::F64, Value::F32(f)) => {
                if nf == 8 {
                    return Err(refuse(format!("extern fn `{}` takes more than eight float arguments", name)));
                }
                // An f32 travels in the low 32 bits of its vector register.
                floats[nf] = if matches!(ty, Type::F32) { f64::from_bits(f.to_bits() as u64) } else { *f as f64 };
                nf += 1;
            }
            (Type::Rawptr(_), _) => {
                return Err(refuse(format!(
                    "extern fn `{}` takes a raw pointer; coby's memory is simulated, so it cannot pass one to C (cobc can)",
                    name
                )))
            }
            _ => return Err(refuse(format!("extern fn `{}`: an argument of type {:?}", name, ty))),
        }
    }
    let kind = match ret {
        Type::F64 => native::Ret::F64,
        Type::F32 => native::Ret::F32,
        Type::Rawptr(_) => {
            return Err(refuse(format!(
                "extern fn `{}` returns a raw pointer; coby's memory is simulated, so it cannot use one from C (cobc can)",
                name
            )))
        }
        _ => native::Ret::Int,
    };
    let Some(f) = native::lookup(name) else {
        return Err(refuse(format!("no C function `{}` in libc or libm", name)));
    };
    let bits = unsafe { native::call(f, &ints, &floats, kind) };
    Ok(match ret {
        Type::Void => Value::Unit,
        Type::Bool => Value::Bool(bits & 0xff != 0),
        Type::F64 => Value::F64(f64::from_bits(bits)),
        Type::F32 => Value::F32(f32::from_bits(bits as u32)),
        Type::Int(t) => {
            let w = t.bitwidth(64);
            let v: i128 = if w >= 64 {
                if t.signed() {
                    bits as i64 as i128
                } else {
                    bits as i128
                }
            } else {
                let masked = bits & ((1u64 << w) - 1);
                if t.signed() && masked >> (w - 1) & 1 == 1 {
                    masked as i128 - (1i128 << w)
                } else {
                    masked as i128
                }
            };
            Value::Int(*t, v)
        }
        _ => return Err(refuse(format!("extern fn `{}`: a result of type {:?}", name, ret))),
    })
}

#[cfg(not(all(unix, target_arch = "x86_64")))]
pub fn call(name: &str, _params: &[Type], _args: &[Value], _ret: &Type) -> Result<Value, Refusal> {
    Err(refuse(format!("extern fn `{}`: coby calls C functions only on x86-64 Linux", name)))
}
