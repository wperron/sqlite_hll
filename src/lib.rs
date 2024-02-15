#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{
    ffi::{c_char, c_uint, CStr, CString},
    mem::{self, size_of},
};

use hyperloglog::HyperLogLog;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[no_mangle]
pub unsafe extern "C" fn sqlite3_sqlitehll_init(
    db: *mut sqlite3,
    _pz_err_msg: *mut *mut ::std::os::raw::c_char,
    p_api: *mut sqlite3_api_routines,
) -> c_uint {
    unsafe {
        faux_sqlite_extension_init2(p_api);
    }

    register_hll(db);

    SQLITE_OK
}

#[derive(Debug, Clone)]
struct SqliteHll {
    inner: HyperLogLog,
}

impl Default for SqliteHll {
    fn default() -> Self {
        Self {
            inner: HyperLogLog::new(0.05),
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct HashableF64 {
    mantissa: u64,
    exponent: i16,
    sign: i8,
}

impl From<f64> for HashableF64 {
    fn from(value: f64) -> Self {
        let (mantissa, exponent, sign) = integer_decode(value);
        Self {
            mantissa,
            exponent,
            sign,
        }
    }
}

fn integer_decode(v: f64) -> (u64, i16, i8) {
    let bits: u64 = unsafe { mem::transmute(v) };
    let sign: i8 = if bits >> 63 == 0 { 1 } else { -1 };
    let mut exponent: i16 = ((bits >> 52) & 0x7ff) as i16;
    let mantissa = if exponent == 0 {
        (bits & 0xfffffffffffff) << 1
    } else {
        (bits & 0xfffffffffffff) | 0x10000000000000
    };
    // Exponent bias + mantissa shift
    exponent -= 1023 + 52;
    (mantissa, exponent, sign)
}

pub unsafe extern "C" fn x_step(
    ctx: *mut sqlite3_context,
    arg_c: i32,
    arg_v: *mut *mut sqlite3_value,
) -> () {
    assert!(arg_c == 1);
    let p: *mut SqliteHll =
        unsafe { sqlite3_aggregate_context(ctx, size_of::<SqliteHll>() as i32) as _ };

    let hll = p.as_mut().expect("couldn't unwrap mut pointer");
    if hll.inner.len().is_nan() {
        // Use the same default error rate as Redis
        // https://redis.io/docs/data-types/probabilistic/hyperloglogs/
        hll.inner = HyperLogLog::new(0.0081);
    }

    unsafe {
        match sqlite3_value_type(*arg_v).try_into().unwrap() {
            SQLITE_NULL => {}
            SQLITE_INTEGER => {
                let x = sqlite3_value_int64(*arg_v);
                hll.inner.insert(&x);
            }
            SQLITE_FLOAT => {
                let x = sqlite3_value_double(*arg_v);
                hll.inner.insert(&HashableF64::from(x));
            }
            SQLITE_TEXT => {
                let s = sqlite3_value_text(*arg_v);
                let cstr = CStr::from_ptr(s as *const i8);
                hll.inner.insert(&cstr)
            }
            SQLITE_BLOB => {
                todo!("implement blob type")
            }
            _ => unreachable!(),
        }
    }

    unsafe {
        *p = hll.to_owned();
    }
}

pub unsafe extern "C" fn x_finalize(ctx: *mut sqlite3_context) -> () {
    let p: *mut SqliteHll = unsafe { sqlite3_aggregate_context(ctx, 0) as _ };
    let hll = p.as_ref().expect("couldn't unwrap mut pointer");

    unsafe {
        sqlite3_result_double(ctx, hll.inner.len());
    }
}

fn register_hll(db: *mut sqlite3) -> () {
    let z_name = CString::new("approx_count_distinct").unwrap();

    unsafe {
        sqlite3_create_function_v2(
            db,
            z_name.as_ptr() as *const c_char,
            1,
            (SQLITE_UTF8 | SQLITE_DETERMINISTIC).try_into().unwrap(),
            std::ptr::null_mut(),
            None,
            Some(x_step),
            Some(x_finalize),
            None,
        );
    }
}

/// Copyright 2022 Alex Garcia. All rights reserved.
/// original: https://github.com/asg017/sqlite-loadable-rs/blob/2c5c049c0c9e010a70b458fde459facf294befce/src/ext.rs#L47
///
/// This function MUST be called in loadable extension before any of the below functions are invoked.
/// (The sqlite_entrypoint function will do this for you).
/// This essentially emulates the SQLITE_EXTENSION_INIT2 macro that's not available in rust-land.
/// Without it, when dynamically loading extensions, calls to SQLite C-API functions in sqlite3ext_sys
/// like sqlite3_value_text will segfault, because sqlite3ext.h does not include their proper definitions.
/// Instead, a sqlite3_api_routines object is provided through the entrypoint at runtime, to which
/// sqlite_loadable will redefine the static SQLITE3_API variable that the functions below requre.
pub unsafe fn faux_sqlite_extension_init2(api: *mut sqlite3_api_routines) {
    if !api.is_null() {
        SQLITE3_API = api;
    }
}

/// If creating a dynmically loadable extension, this MUST be redefined to point
/// to a proper sqlite3_api_rountines module (from a entrypoint function).
/// The "sqlite_entrypoint" macro will do this for you usually.
static mut SQLITE3_API: *mut sqlite3_api_routines = std::ptr::null_mut();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity() {
        assert_eq!(SQLITE_VERSION_NUMBER, 3042000);
        assert_eq!(unsafe { sqlite3_libversion_number() }, 3042000);
    }
}
