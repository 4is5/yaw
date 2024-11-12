use std::os::raw::{c_double, c_uint};

extern "C" {
    pub fn emscripten_get_now() -> c_double;
    pub fn emscripten_sleep(millis: c_uint);
}
