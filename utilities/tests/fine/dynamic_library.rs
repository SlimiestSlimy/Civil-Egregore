//! A dynamic library opened, and a function of it called.

use std::ffi::{c_char, CStr};
use std::path::Path;
use utilities::dynamic_library::DynamicLibrary;

/// The system's C library opened, its `strlen` found and called; a
/// library not there, and a function not in it, refused.
#[test]
fn a_library_opened_and_a_function_of_it_called() {
    let c_library = DynamicLibrary::open(Path::new("libc.so.6")).expect("the C library");
    // SAFETY: strlen is so in the C library.
    let strlen = unsafe { c_library.function::<unsafe extern "C" fn(*const c_char) -> usize>("strlen") }.expect("strlen");
    let text: &CStr = c"seven c";
    // SAFETY: a C string.
    assert_eq!(unsafe { strlen(text.as_ptr()) }, 7);
    assert!(DynamicLibrary::open(Path::new("no_such_library.so")).is_err());
    // SAFETY: never called.
    assert!(unsafe { c_library.function::<fn()>("no_such_function") }.is_err());
}
