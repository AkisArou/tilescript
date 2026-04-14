use std::ffi::{CStr, CString, c_char};

use crate::response::FfiError;

pub fn cstr_to_str<'a>(value: *const c_char) -> Result<&'a str, FfiError> {
    if value.is_null() {
        return Err(FfiError::NullPointer);
    }

    let cstr = unsafe { CStr::from_ptr(value) };
    cstr.to_str().map_err(|error| FfiError::InvalidUtf8(error.to_string()))
}

pub fn optional_cstr_to_string(value: *const c_char) -> Result<Option<String>, FfiError> {
    if value.is_null() {
        return Ok(None);
    }

    Ok(Some(cstr_to_str(value)?.to_string()))
}

pub unsafe fn string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    unsafe {
        drop(CString::from_raw(value));
    }
}
