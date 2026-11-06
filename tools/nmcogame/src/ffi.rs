//! The exported functions (cdecl, the ordinals of exports.def). Only module with `unsafe`: each
//! block reads or writes a pointer the engine passed, after a null check, within the length the
//! engine gave (the call envelope's own length field, the reply buffer's capacity).
use std::ffi::c_char;

/// Largest call envelope read for the log: the engine's reply buffer is 1 MB.
const MAX_CALL: u32 = 0x10_0000;

/// NMCO_CallNMFunction(code, callBuf, cryptVer, ppRet, pRetLen): BOOL.
#[unsafe(no_mangle)]
pub extern "C" fn NMCO_CallNMFunction(
    code: u32,
    call: *const u8,
    crypt: u8,
    reply: *mut *mut u8,
    reply_len: *mut u32,
) -> i32 {
    let envelope: Vec<u8> = if call.is_null() {
        Vec::new()
    } else {
        // SAFETY: the envelope starts with u32 magic and u32 length; the engine built it as
        // magic + length + body + trailer, so 8 + len + 4 bytes are readable.
        let header = unsafe { std::slice::from_raw_parts(call, 8) };
        let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]).min(MAX_CALL);
        let total = 8 + len as usize + 4;
        // SAFETY: as above, the whole envelope belongs to the engine's call buffer.
        unsafe { std::slice::from_raw_parts(call, total) }.to_vec()
    };
    let answer = crate::answer(code);
    crate::log(&crate::log_line(code, crypt, &envelope, answer.is_some()));
    let Some(bytes) = answer else {
        return 0;
    };
    if reply.is_null() || reply_len.is_null() {
        return 0;
    }
    // SAFETY: CNMCOClientObject::CallNMFunc passes a preallocated buffer in *ppRet and its
    // capacity in *pRetLen; the reply is written only when it fits, and the pointer is kept
    // (so the engine does not call NMCO_MemoryFree on it).
    unsafe {
        let buffer = *reply;
        if buffer.is_null() || (*reply_len as usize) < bytes.len() {
            return 0;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
        *reply_len = bytes.len() as u32;
    }
    1
}

/// NMCO_MemoryFree: this DLL never hands out its own allocations.
#[unsafe(no_mangle)]
pub extern "C" fn NMCO_MemoryFree(_memory: *mut u8) {}

/// NMCO_SetLocaleAndRegion(locale, region, pErr, pDetail): TRUE, no error.
#[unsafe(no_mangle)]
pub extern "C" fn NMCO_SetLocaleAndRegion(
    locale: u32,
    region: u32,
    error: *mut u32,
    detail: *mut u32,
) -> i32 {
    crate::log(&format!("set_locale_and_region locale={locale} region={region}"));
    // SAFETY: optional out parameters, written only when present.
    unsafe {
        if !error.is_null() {
            *error = 0;
        }
        if !detail.is_null() {
            *detail = 0;
        }
    }
    1
}

/// NMCO_SetPatchOption(option): TRUE.
#[unsafe(no_mangle)]
pub extern "C" fn NMCO_SetPatchOption(option: u32) -> i32 {
    crate::log(&format!("set_patch_option option={option}"));
    1
}

/// NMCO_SetVersionFileUrlA(url): TRUE (the patcher is not used).
#[unsafe(no_mangle)]
pub extern "C" fn NMCO_SetVersionFileUrlA(_url: *const c_char) -> i32 {
    crate::log("set_version_file_url_a");
    1
}

/// NMCO_SetVersionFileUrlW(url): TRUE.
#[unsafe(no_mangle)]
pub extern "C" fn NMCO_SetVersionFileUrlW(_url: *const u16) -> i32 {
    crate::log("set_version_file_url_w");
    1
}
