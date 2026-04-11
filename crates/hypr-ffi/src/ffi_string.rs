use std::ffi::{c_char, CStr, CString};

use crate::response::{fallback_json, response_err, FfiError};

pub fn cstr_to_str<'a>(value: *const c_char) -> Result<&'a str, FfiError> {
    if value.is_null() {
        return Err(FfiError::NullPointer);
    }

    let cstr = unsafe { CStr::from_ptr(value) };
    cstr.to_str()
        .map_err(|error| FfiError::InvalidUtf8(error.to_string()))
}

pub fn optional_cstr_to_string(value: *const c_char) -> Result<Option<String>, FfiError> {
    if value.is_null() {
        return Ok(None);
    }

    Ok(Some(cstr_to_str(value)?.to_string()))
}

pub fn into_ffi_string(result: std::thread::Result<Result<String, FfiError>>) -> *mut c_char {
    let json = match result {
        Ok(Ok(json)) => json,
        Ok(Err(error)) => match response_err(error) {
            Ok(json) => json,
            Err(fallback_error) => fallback_json(fallback_error),
        },
        Err(_) => match response_err(FfiError::Panic) {
            Ok(json) => json,
            Err(fallback_error) => fallback_json(fallback_error),
        },
    };

    match CString::new(json) {
        Ok(value) => value.into_raw(),
        Err(error) => CString::new(fallback_json(FfiError::NulByte(error.to_string())))
            .expect("fallback ffi json must not contain nul bytes")
            .into_raw(),
    }
}

pub unsafe fn string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    unsafe {
        drop(CString::from_raw(value));
    }
}
