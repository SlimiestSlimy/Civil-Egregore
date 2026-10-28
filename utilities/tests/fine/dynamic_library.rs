//! A dynamic library opened, and a static of it read.

use std::ffi::c_int;
use std::path::Path;
use utilities::dynamic_library::DynamicLibrary;

/// The system's C library opened, its `opterr` found and read -- 1 until
/// a program sets it; a library not there, and a static not in it,
/// refused.
#[test]
fn a_library_opened_and_a_static_of_it_read() {
    let c_library = DynamicLibrary::open(Path::new("libc.so.6")).expect("the C library");
    // SAFETY: opterr is an int in the C library.
    assert_eq!(unsafe { c_library.static_value::<c_int>("opterr") }, Ok(1));
    assert!(DynamicLibrary::open(Path::new("no_such_library.so")).is_err());
    // SAFETY: never read.
    assert!(unsafe { c_library.static_value::<c_int>("no_such_static") }.is_err());
}
