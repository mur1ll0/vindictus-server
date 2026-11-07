//! The exported functions (cdecl, the ordinals of exports.def). Only module with `unsafe`: each
//! block reads or writes a pointer the engine passed, after a null check, within the length the
//! engine gave (the call envelope's own length field, the reply buffer's capacity), posts an
//! event with PostMessageW, or frees a buffer this module handed out.
use std::{
    collections::HashMap,
    ffi::c_char,
    sync::{Mutex, PoisonError},
};

#[link(name = "user32")]
unsafe extern "system" {
    fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
}

/// Buffers handed to the engine (address → length): replies larger than its buffer and event
/// envelopes, released by NMCO_MemoryFree.
static HANDED: Mutex<Option<HashMap<usize, usize>>> = Mutex::new(None);

/// Moves a non-empty buffer to the engine; the address goes back to `release`.
fn hand_out(bytes: Vec<u8>) -> *mut u8 {
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len();
    let pointer = Box::into_raw(boxed).cast::<u8>();
    HANDED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(pointer as usize, len);
    pointer
}

/// Frees a buffer from `hand_out`; other addresses are ignored.
fn release(pointer: *mut u8) {
    let len = HANDED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .and_then(|m| m.remove(&(pointer as usize)));
    if let Some(len) = len {
        // SAFETY: `pointer` and `len` come from Box::into_raw of a boxed slice in `hand_out`,
        // removed from the map so it is released once.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(pointer, len)) });
    }
}

/// Posts an event envelope to a window: wParam = event code, lParam = the buffer, which the
/// engine returns through NMCO_MemoryFree after handling it.
fn post(hwnd: u32, msg: u32, code: u32, envelope: Vec<u8>) -> bool {
    if envelope.is_empty() || hwnd == 0 {
        return false;
    }
    let buffer = hand_out(envelope);
    // SAFETY: plain Win32 call with a window handle the engine registered; the buffer stays
    // valid until NMCO_MemoryFree.
    let posted = unsafe { PostMessageW(hwnd as isize, msg, code as usize, buffer as isize) } != 0;
    if !posted {
        release(buffer);
    }
    posted
}

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
    crate::set_post(post);
    let answer = crate::dispatch(code, &envelope);
    crate::log(&crate::log_line(code, crypt, &envelope, answer.is_some()));
    let Some(bytes) = answer else {
        return 0;
    };
    if reply.is_null() || reply_len.is_null() || bytes.is_empty() {
        return 0;
    }
    // SAFETY: CNMCOClientObject::CallNMFunc passes a preallocated buffer in *ppRet and its
    // capacity in *pRetLen. A reply that fits is copied there (the pointer is kept, so the
    // engine does not free it); a larger one replaces the pointer with a buffer of this module,
    // which the engine returns through NMCO_MemoryFree.
    unsafe {
        let buffer = *reply;
        if !buffer.is_null() && (*reply_len as usize) >= bytes.len() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
            *reply_len = bytes.len() as u32;
        } else {
            let len = bytes.len() as u32;
            *reply = hand_out(bytes);
            *reply_len = len;
        }
    }
    1
}

/// NMCO_MemoryFree: releases the replies and event buffers this DLL handed out.
#[unsafe(no_mangle)]
pub extern "C" fn NMCO_MemoryFree(memory: *mut u8) {
    if !memory.is_null() {
        release(memory);
    }
}

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
