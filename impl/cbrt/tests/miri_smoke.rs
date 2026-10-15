// D-0264 (plan B4): the runtime's table paths under Miri. A Rust test
// plays the generated program: it defines the two symbols a program
// defines, registers one file and one scalar type, and drives a frame
// through the steps a statement takes — an object established over a
// local, bound, borrowed, read through the borrow, written through its
// root, and ended with its frame — so that Miri checks the runtime's
// unsafe code on those paths for undefined behaviour.
//     cargo +nightly miri test -p cbrt --test miri_smoke
// It also runs as an ordinary test (`cargo test -p cbrt`).
use std::ffi::CString;
use std::ptr;

#[no_mangle]
pub extern "C" fn cb_where(file: *mut u32, line: *mut u32) {
    unsafe {
        *file = u32::MAX;
        *line = 0;
    }
}

#[no_mangle]
pub extern "C" fn cb_set_where(_file: u32, _line: u32) {}

#[no_mangle]
pub extern "C" fn cb_set_stack_floor(_floor: usize) {}

#[test]
fn a_statement_s_steps_leave_no_undefined_behaviour() {
    let file = CString::new("t.cb").unwrap();
    let files = [file.as_ptr()];
    let name = CString::new("i32").unwrap();
    let ty = cbrt::CbType {
        name: name.as_ptr(),
        size: 4,
        kind: 0,
        is_resource: 0,
        has_refs: 0,
        drop: None,
        nfields: 0,
        fields: ptr::null(),
        nvariants: 0,
        variants: ptr::null(),
        payload_off: 0,
        elem: 0,
        count: 0,
        owner: 0,
    };
    let ids = [1u32];
    unsafe { cbrt::cb_init(files.as_ptr(), 1, &ty, 1, ids.as_ptr()) };
    cbrt::cb_frame_push();
    // `i32 x = 5;`
    cbrt::cb_stmt_push_at(0, 1);
    let mut x: i32 = 5;
    let obj = cbrt::cb_new(&mut x as *mut i32 as *mut u8, 1, 0, 1);
    let root = cbrt::cb_bind(obj);
    cbrt::cb_stmt_pop();
    // `i32 y = *(&x);`: a shared borrow, read through, ending with the statement.
    cbrt::cb_stmt_push_at(0, 2);
    let r = unsafe { cbrt::cb_borrow(root, ptr::null(), 0, 0, 0, 2) };
    unsafe { cbrt::cb_read(r, ptr::null(), 0, 0, 2) };
    let y = x;
    cbrt::cb_stmt_pop();
    // `x = y + 1;`
    cbrt::cb_stmt_push_at(0, 3);
    unsafe { cbrt::cb_write(root, ptr::null(), 0, 0, 0, 3) };
    x = y + 1;
    cbrt::cb_stmt_pop();
    assert_eq!(x, 6);
    cbrt::cb_frame_pop();
}
