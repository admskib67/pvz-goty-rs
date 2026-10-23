//! A small safe wrapper over libopenmpt's C API, enough to play PvZ's music:
//! load a module, render it, jump to an order/row and set channel volumes.
//!
//! libopenmpt is linked statically from prebuilt libraries in
//! `dev/libopenmpt-<os>/` (see `dev/README.md`), so nothing has to be
//! installed on the player's machine.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

#[repr(C)]
struct RawModule {
    _private: [u8; 0],
}

#[repr(C)]
struct RawModuleExt {
    _private: [u8; 0],
}

type LogFunc = Option<unsafe extern "C" fn(*const c_char, *mut c_void)>;
type ErrorFunc = Option<unsafe extern "C" fn(c_int, *mut c_void) -> c_int>;

/// `openmpt_module_ext_interface_interactive`; field order must match the header.
#[repr(C)]
#[derive(Default)]
struct Interactive {
    set_current_speed: Option<unsafe extern "C" fn(*mut RawModuleExt, i32) -> c_int>,
    set_current_tempo: Option<unsafe extern "C" fn(*mut RawModuleExt, i32) -> c_int>,
    set_tempo_factor: Option<unsafe extern "C" fn(*mut RawModuleExt, f64) -> c_int>,
    get_tempo_factor: Option<unsafe extern "C" fn(*mut RawModuleExt) -> f64>,
    set_pitch_factor: Option<unsafe extern "C" fn(*mut RawModuleExt, f64) -> c_int>,
    get_pitch_factor: Option<unsafe extern "C" fn(*mut RawModuleExt) -> f64>,
    set_global_volume: Option<unsafe extern "C" fn(*mut RawModuleExt, f64) -> c_int>,
    get_global_volume: Option<unsafe extern "C" fn(*mut RawModuleExt) -> f64>,
    set_channel_volume: Option<unsafe extern "C" fn(*mut RawModuleExt, i32, f64) -> c_int>,
    get_channel_volume: Option<unsafe extern "C" fn(*mut RawModuleExt, i32) -> f64>,
    set_channel_mute_status: Option<unsafe extern "C" fn(*mut RawModuleExt, i32, c_int) -> c_int>,
    get_channel_mute_status: Option<unsafe extern "C" fn(*mut RawModuleExt, i32) -> c_int>,
    set_instrument_mute_status: Option<unsafe extern "C" fn(*mut RawModuleExt, i32, c_int) -> c_int>,
    get_instrument_mute_status: Option<unsafe extern "C" fn(*mut RawModuleExt, i32) -> c_int>,
    play_note: Option<unsafe extern "C" fn(*mut RawModuleExt, i32, i32, f64, f64) -> i32>,
    stop_note: Option<unsafe extern "C" fn(*mut RawModuleExt, i32) -> c_int>,
}

unsafe extern "C" {
    fn openmpt_get_library_version() -> u32;
    fn openmpt_log_func_silent(message: *const c_char, user: *mut c_void);
    fn openmpt_error_string(error: c_int) -> *const c_char;
    fn openmpt_free_string(s: *const c_char);
    fn openmpt_module_ext_create_from_memory(
        filedata: *const c_void,
        filesize: usize,
        logfunc: LogFunc,
        loguser: *mut c_void,
        errfunc: ErrorFunc,
        erruser: *mut c_void,
        error: *mut c_int,
        error_message: *mut *const c_char,
        ctls: *const c_void,
    ) -> *mut RawModuleExt;
    fn openmpt_module_ext_destroy(m: *mut RawModuleExt);
    fn openmpt_module_ext_get_module(m: *mut RawModuleExt) -> *mut RawModule;
    fn openmpt_module_ext_get_interface(
        m: *mut RawModuleExt,
        id: *const c_char,
        interface: *mut c_void,
        size: usize,
    ) -> c_int;
    fn openmpt_module_set_repeat_count(m: *mut RawModule, repeat_count: i32) -> c_int;
    fn openmpt_module_set_position_order_row(m: *mut RawModule, order: i32, row: i32) -> f64;
    fn openmpt_module_read_interleaved_float_stereo(
        m: *mut RawModule,
        samplerate: i32,
        count: usize,
        interleaved: *mut f32,
    ) -> usize;
    fn openmpt_module_get_current_order(m: *mut RawModule) -> i32;
    fn openmpt_module_get_current_row(m: *mut RawModule) -> i32;
    fn openmpt_module_get_num_channels(m: *mut RawModule) -> i32;
    fn openmpt_module_get_num_orders(m: *mut RawModule) -> i32;
}

/// libopenmpt's version as (major, minor, patch).
pub fn library_version() -> (u32, u32, u32) {
    let v = unsafe { openmpt_get_library_version() };
    (v >> 24, (v >> 16) & 0xff, v & 0xffff)
}

/// A loaded tracker module (MOD/XM/S3M/IT/MO3/...).
pub struct Module {
    ext: *mut RawModuleExt,
    module: *mut RawModule,
    interactive: Interactive,
}

// libopenmpt modules aren't shared between threads, only moved, which is safe.
unsafe impl Send for Module {}

impl Module {
    pub fn from_memory(data: &[u8]) -> Result<Self, String> {
        let mut error: c_int = 0;
        let mut message: *const c_char = ptr::null();
        let ext = unsafe {
            openmpt_module_ext_create_from_memory(
                data.as_ptr().cast(),
                data.len(),
                Some(openmpt_log_func_silent),
                ptr::null_mut(),
                None,
                ptr::null_mut(),
                &mut error,
                &mut message,
                ptr::null(),
            )
        };
        if ext.is_null() {
            let text = unsafe {
                let s = if message.is_null() { openmpt_error_string(error) } else { message };
                let text = CStr::from_ptr(s).to_string_lossy().into_owned();
                openmpt_free_string(s);
                text
            };
            return Err(format!("libopenmpt: {text}"));
        }

        let mut interactive = Interactive::default();
        let found = unsafe {
            openmpt_module_ext_get_interface(
                ext,
                c"interactive".as_ptr(),
                (&mut interactive as *mut Interactive).cast(),
                size_of::<Interactive>(),
            )
        };
        let module = unsafe { openmpt_module_ext_get_module(ext) };
        let m = Self { ext, module, interactive };
        if found == 0 {
            return Err("libopenmpt: interactive interface unavailable".into());
        }
        Ok(m)
    }

    /// -1 loops forever, 0 plays once.
    pub fn set_repeat_count(&mut self, count: i32) {
        unsafe { openmpt_module_set_repeat_count(self.module, count) };
    }

    /// Jumps to a pattern row in the order list. Returns the new position in seconds.
    pub fn set_position_order_row(&mut self, order: i32, row: i32) -> f64 {
        unsafe { openmpt_module_set_position_order_row(self.module, order, row) }
    }

    /// Renders interleaved stereo; returns the number of frames written
    /// (fewer than requested only at the end of the song).
    pub fn read_stereo(&mut self, sample_rate: i32, out: &mut [f32]) -> usize {
        unsafe { openmpt_module_read_interleaved_float_stereo(self.module, sample_rate, out.len() / 2, out.as_mut_ptr()) }
    }

    pub fn current_order(&self) -> i32 {
        unsafe { openmpt_module_get_current_order(self.module) }
    }

    pub fn current_row(&self) -> i32 {
        unsafe { openmpt_module_get_current_row(self.module) }
    }

    pub fn num_channels(&self) -> i32 {
        unsafe { openmpt_module_get_num_channels(self.module) }
    }

    pub fn num_orders(&self) -> i32 {
        unsafe { openmpt_module_get_num_orders(self.module) }
    }

    /// Channel volume, 0..=1.
    pub fn set_channel_volume(&mut self, channel: i32, volume: f64) {
        if let Some(f) = self.interactive.set_channel_volume {
            unsafe { f(self.ext, channel, volume) };
        }
    }

    pub fn channel_volume(&self, channel: i32) -> f64 {
        self.interactive.get_channel_volume.map_or(1.0, |f| unsafe { f(self.ext, channel) })
    }

    /// Global volume, 0..=1.
    pub fn set_global_volume(&mut self, volume: f64) {
        if let Some(f) = self.interactive.set_global_volume {
            unsafe { f(self.ext, volume) };
        }
    }

    pub fn set_tempo_factor(&mut self, factor: f64) {
        if let Some(f) = self.interactive.set_tempo_factor {
            unsafe { f(self.ext, factor) };
        }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        unsafe { openmpt_module_ext_destroy(self.ext) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_vendored_version() {
        assert_eq!(library_version(), (0, 8, 9));
    }

    #[test]
    fn rejects_garbage() {
        assert!(Module::from_memory(b"definitely not a module").is_err());
    }
}
