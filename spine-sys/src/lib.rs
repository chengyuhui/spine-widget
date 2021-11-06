#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::os::raw::{c_char, c_int};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn _spUtil_readFile(path: *const c_char, length: *mut c_int) -> *mut c_char {
    _readFile(path, length)
}
