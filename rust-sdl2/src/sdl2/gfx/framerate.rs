//! Framerate control

use get_error;
use std::ffi;
use std::ffi::{c_void};
use std::mem;
use sys::gfx;

/// Structure holding the state and timing information of the framerate controller.
pub struct FPSManager {
    raw: *mut gfx::framerate::FPSmanager,
}

impl FPSManager {
    /// Create the framerate manager.
    pub fn new() -> FPSManager {
        unsafe {
            let size = mem::size_of::<gfx::framerate::FPSmanager>() as usize;
            let raw = std::ffi::malloc(size) as *mut gfx::framerate::FPSmanager;
            gfx::framerate::SDL_initFramerate(raw);
            FPSManager { raw }
        }
    }

    /// Set the framerate in Hz.
    pub fn set_framerate(&mut self, rate: u32) -> Result<(), String> {
        let ret = unsafe { gfx::framerate::SDL_setFramerate(self.raw, rate) };
        match ret {
            0 => Ok(()),
            _ => Err(get_error()),
        }
    }

    /// Return the current target framerate in Hz.
    pub fn get_framerate(&self) -> i32 {
        // will not get an error
        unsafe { gfx::framerate::SDL_getFramerate(self.raw) as i32 }
    }

    /// Return the current framecount.
    pub fn get_frame_count(&self) -> i32 {
        // will not get an error
        unsafe { gfx::framerate::SDL_getFramecount(self.raw) as i32 }
    }

    /// Delay execution to maintain a constant framerate and calculate fps.
    pub fn delay(&mut self) -> u32 {
        unsafe { gfx::framerate::SDL_framerateDelay(self.raw) as u32 }
    }
}

impl Drop for FPSManager {
    fn drop(&mut self) {
        unsafe { std::ffi::free(self.raw as *mut c_void) }
    }
}
