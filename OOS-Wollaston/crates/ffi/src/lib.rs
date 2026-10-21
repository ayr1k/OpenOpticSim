//! C ABI v1. JSON UTF-8 in, envelope JSON out. See include/wollaston.h.
use std::{
    ffi::{c_char, CStr, CString},
    sync::atomic::{AtomicBool, Ordering},
};
#[no_mangle]
pub extern "C" fn wollaston_abi_version() -> u32 {
    1
}
#[no_mangle]
pub extern "C" fn wollaston_cancel_new() -> *mut AtomicBool {
    Box::into_raw(Box::new(AtomicBool::new(false)))
}
/// # Safety
/// Token must be live from cancel_new. May run concurrently with execute.
#[no_mangle]
pub unsafe extern "C" fn wollaston_cancel(token: *const AtomicBool) {
    if let Some(token) = token.as_ref() {
        token.store(true, Ordering::Relaxed);
    }
}
/// # Safety
/// Free exactly once, after all executing calls have returned.
#[no_mangle]
pub unsafe extern "C" fn wollaston_cancel_free(token: *mut AtomicBool) {
    if !token.is_null() {
        drop(Box::from_raw(token));
    }
}
/// # Safety
/// Input must be readable NUL-terminated UTF-8, token null or live until return.
/// The caller frees the returned pointer with wollaston_free exactly once.
#[no_mangle]
pub unsafe extern "C" fn wollaston_execute(
    input: *const c_char,
    token: *const AtomicBool,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> Result<serde_json::Value, String> {
        if input.is_null() {
            return Err("null request".into());
        }
        let input = CStr::from_ptr(input).to_str().map_err(|e| e.to_string())?;
        if input.len() > 4_000_000 {
            return Err("request exceeds 4 MB".into());
        }
        let request = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let local = AtomicBool::new(false);
        wollaston::execute(request, token.as_ref().unwrap_or(&local))
    });
    let envelope = match result {
        Ok(Ok(value)) => serde_json::json!({"ok":true,"result":value}),
        Ok(Err(error)) => serde_json::json!({"ok":false,"error":error}),
        Err(_) => serde_json::json!({"ok":false,"error":"internal panic"}),
    };
    CString::new(envelope.to_string()).unwrap().into_raw()
}
/// # Safety
/// Pointer must be null or an unfreed result returned by wollaston_execute.
#[no_mangle]
pub unsafe extern "C" fn wollaston_free(result: *mut c_char) {
    if !result.is_null() {
        drop(CString::from_raw(result));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_and_null_are_owned_errors() {
        unsafe {
            for p in [std::ptr::null(), c"{".as_ptr()] {
                let out = wollaston_execute(p, std::ptr::null());
                let v: serde_json::Value =
                    serde_json::from_str(CStr::from_ptr(out).to_str().unwrap()).unwrap();
                assert_eq!(v["ok"], false);
                wollaston_free(out);
            }
            let token = wollaston_cancel_new();
            wollaston_cancel(token);
            assert!((*token).load(Ordering::Relaxed));
            wollaston_cancel_free(token);
        }
    }
}
