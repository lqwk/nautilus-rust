use core::{
    marker::PhantomData,
    ffi::{c_int, c_void, c_char, CStr},
};

use alloc::{sync::Arc, boxed::Box, ffi::CString};

use crate::kernel::{
    error::{Result, ResultExt},
    print::make_logging_macros,
    bindings
};

make_logging_macros!("gpudev");

/// Manages resources associated with registering a GPU
/// device.
/// 
/// # Invariants
///
/// `dev` and `data` are valid, non-null pointers.
#[derive(Debug)]
struct InternalRegistration<T> {
    name: CString,
    dev: *mut bindings::nk_gpu_dev,
    data: *mut c_void,
    _p: PhantomData<Arc<T>>,
}

// SAFETY: `dev` and `data` are raw pointers with no thread affinity. The C
// code using them does not modify them or move
// their referrents. We only store them in an `_InternalRegistration`
// so that we can later reclaim the memory they point to. So it is
// safe to send an `_InternalRegistration` between threads. `Send`
// is important to implement here so that, if some type `T` contains
// an `chardev::Registration`, then `Mutex<NkIrqLock, T>` (from `lock_api`)
// implements `Sync` and `Send`.
//
// This is one of those `unsafe` lines that I cannot say with 100%
// confidence are actually safe. See also the comment for
// `irq::_InternalRegistration`.
unsafe impl<T> Send for InternalRegistration<T> {}

impl<T> InternalRegistration<T> {
    /// Registers a GPU device with Nautilus' GPU device subsytem.
    unsafe fn try_new(
        name: &str,
        interface: *mut bindings::nk_gpu_dev_int,
        data: Arc<T>,
    ) -> Result<Self> {
        let c_name = match CString::new(name) {
            Ok(s) => s,
            Err(_) => {
                error!("Cannot create C string from {name}.");
                return Err(-1)
            }
        };

        let ptr = Arc::into_raw(data) as *mut c_void;

        let dev = 
            // SAFETY: `name` will never be written to by the C code, and
            // will remain valid as long as the registration is alive.
            // Similarly, `ptr` also remains valid as long as the
            // registration is alive.
            unsafe {
                bindings::nk_gpu_dev_register(
                    c_name.as_ptr() as *mut c_char,
                    0,
                    interface,
                    ptr as *mut c_void,
                )
            };

        if dev.is_null() {
            error!("Unable to register device {}.", name);
            // SAFETY: `ptr` came from a previous call to `into_raw`.
            unsafe { let _ = Arc::from_raw(ptr as *mut T); }
            Err(-1)
        } else {
            debug!("Successfully registered device {}.", name);
            Ok(Self {
                name: c_name,
                dev,
                data: ptr,
                _p: PhantomData,
            })
        }
    }
}

impl<T> Drop for InternalRegistration<T> {
    fn drop(&mut self) {
        debug!("Dropping a registration for device {:?}.", self.name);

        // SAFETY: `self.dev` was successfully registered when the
        // registration was created, so it is non-null and safe to
        // deregister.
        unsafe { bindings::nk_gpu_dev_unregister(self.dev); }

        // SAFETY: This matches the call to `into_raw` from `try_new`
        // in the success case.
        unsafe { Arc::from_raw(self.data as *mut T); }
    }
}

pub type VideoMode = bindings::nk_gpu_dev_video_mode_t;

pub type Coordinate = bindings::nk_gpu_dev_coordinate_t;

pub type Char = bindings::nk_gpu_dev_char_t;

pub type Rect = bindings::nk_gpu_dev_box_t;

pub type Region = bindings::nk_gpu_dev_region_t;

pub type Pixel = bindings::nk_gpu_dev_pixel_t;

pub type BitBlitOp = bindings::nk_gpu_dev_bit_blit_op_t;

pub type Bitmap = bindings::nk_gpu_dev_bitmap_t;

pub type Font = bindings::nk_gpu_dev_font_t;


/// A Nautilus GPU device.
pub trait GpuDev {
    /// The state associated with the GPU device.
    type State: Send + Sync;

    // gpudev-specific interface - set to zero if not available
    // an interface either succeeds (returns zero) or fails (returns -1)

    // discover the modes supported by the device
    //     modes = array of modes on entry, filled out on return
    //     num = size of array on entry, number of modes found on return
    // 
    fn get_available_modes(state: &Self::State, modes: &mut [VideoMode]) -> Result<usize>;

    // grab the current mode - useful in case you need to reset it later
    fn get_mode(state: &Self::State) -> Result<VideoMode>;
    
    // set a video mode based on the modes discovered
    // this will switch to the mode before returning
    fn set_mode(state: &Self::State, mode: &VideoMode) -> Result;

    // drawing commands
    
    // each of these is asynchronous - the implementation should start the operation
    // but not necessarily finish it.   In particular, nothing needs to be drawn
    // until flush is invoked

    // flush - wait until all preceding drawing commands are visible by the user
    fn flush(state: &Self::State) -> Result;

    fn clip_apply_with_blit(state: &Self::State, location: &Coordinate, oldpixel: &mut Pixel, newpixel: &Pixel, op: BitBlitOp) -> Result;

    // text mode drawing commands
    fn text_set_char(state: &Self::State, location: &Coordinate, val: &Char) -> Result;

    // cursor location in text mode
    fn text_set_cursor(state: &Self::State, location: &Coordinate, flags: u32) -> Result;

    // graphics mode drawing commands
    // confine drawing to this box or region
    fn graphics_set_clipping_box(state: &Self::State, rect: Option<&Rect>) -> Result;

    fn graphics_set_clipping_region(state: &Self::State, region: &Region) -> Result;

    // draw stuff 
    fn graphics_draw_pixel(state: &Self::State, location: &Coordinate, pixel: &Pixel) -> Result;
    fn graphics_draw_line(state: &Self::State, start: &Coordinate, end: &Coordinate, pixel: &Pixel) -> Result;
    fn graphics_draw_poly(state: &Self::State, coord_list: &[Coordinate], pixel: &Pixel) -> Result;
    fn graphics_fill_box_with_pixel(state: &Self::State, rect: &Rect, pixel: &Pixel, op: BitBlitOp) -> Result;
    fn graphics_fill_box_with_bitmap(state: &Self::State, rect: &Rect, bitmap: &Bitmap, op: BitBlitOp) -> Result;
    fn graphics_copy_box(state: &Self::State, source_rect: &Rect, dest_box: &Rect, op: BitBlitOp) -> Result;
    fn graphics_draw_text(state: &Self::State, location: &Coordinate, font: &Font, text: &str) -> Result;

    // mouse functions, if supported
    fn graphics_set_cursor_bitmap(state: &Self::State, bitmap: &Bitmap) -> Result;
    // the location is the position of the top-left pixel in the bitmap
    fn graphics_set_cursor(state: &Self::State, location: &Coordinate) -> Result;
}

/// The registration of a GPU device.
#[derive(Debug)]
pub struct Registration<G: GpuDev>(InternalRegistration<G::State>);

impl<G: GpuDev> Registration<G> {
    unsafe extern "C" fn get_available_modes(
        raw_state: *mut c_void,
        raw_modes: *mut bindings::nk_gpu_dev_video_mode_t,
        num: *mut u32,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller (GPU subsystem) ensures `raw_modes` is a pointer to `*num` modes.
        let modes = unsafe { core::slice::from_raw_parts_mut(raw_modes, *num as usize) };

        match G::get_available_modes(state, modes) {
            Ok(n) => {
                // SAFETY: Caller guarantees `num` is a valid pointer. Caller expects
                // us to overwrite it's value before returning with the number of modes
                // we found.
                unsafe { *num = n as u32; }
                0
            },
            Err(v) => v
        }
    }

    unsafe extern "C" fn get_mode(
        raw_state: *mut c_void,
        raw_mode: *mut bindings::nk_gpu_dev_video_mode_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        let ret = G::get_mode(state);

        if let Ok(mode) = ret {
            // SAFETY: Caller ensures `raw_mode` is a valid pointer.
            unsafe { *raw_mode = mode; }
        }

        ret.map(|_| ()).as_error_code()
    }

    unsafe extern "C" fn set_mode(
        raw_state: *mut c_void,
        raw_mode: *mut bindings::nk_gpu_dev_video_mode_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_mode` is a valid pointer.
        let mode = unsafe { &*raw_mode };

        G::set_mode(state, mode).as_error_code()
    }

    unsafe extern "C" fn flush(raw_state: *mut c_void) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        G::flush(state).as_error_code()
    }

    unsafe extern "C" fn text_set_char(
        raw_state: *mut c_void,
        raw_location: *mut bindings::nk_gpu_dev_coordinate_t,
        raw_val: *mut bindings::nk_gpu_dev_char_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_location` is a valid pointer.
        let location = unsafe { &*raw_location };

        // SAFETY: Caller ensures `raw_val` is a valid pointer.
        let val = unsafe { &*raw_val };

        G::text_set_char(state, location, val).as_error_code()
    }

    unsafe extern "C" fn text_set_cursor(
        raw_state: *mut c_void,
        raw_location: *mut bindings::nk_gpu_dev_coordinate_t,
        flags: u32,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_location` is a valid pointer.
        let location = unsafe { &*raw_location };

        G::text_set_cursor(state, location, flags).as_error_code()
    }

    unsafe extern "C" fn graphics_set_clipping_box(
        raw_state: *mut c_void,
        raw_rect: *mut bindings::nk_gpu_dev_box_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_rect` is a valid pointer.
        let rect = unsafe { raw_rect.as_ref() };

        G::graphics_set_clipping_box(state, rect).as_error_code()
    }

    unsafe extern "C" fn graphics_set_clipping_region(
        raw_state: *mut c_void,
        raw_region: *mut bindings::nk_gpu_dev_region_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensuers `raw_region` is a valid pointer.
        let region = unsafe { &*raw_region };

        G::graphics_set_clipping_region(state, region).as_error_code()
    }

    unsafe extern "C" fn graphics_draw_pixel(
        raw_state: *mut c_void,
        raw_location: *mut bindings::nk_gpu_dev_coordinate_t,
        raw_pixel: *mut bindings::nk_gpu_dev_pixel_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensuers `raw_location` is a valid pointer.
        let location = unsafe { &*raw_location };

        // SAFETY: Caller ensuers `raw_pixel` is a valid pointer.
        let pixel = unsafe { &*raw_pixel };

        G::graphics_draw_pixel(state, location, pixel).as_error_code()
    }

    unsafe extern "C" fn graphics_draw_line(
        raw_state: *mut c_void,
        raw_start: *mut bindings::nk_gpu_dev_coordinate_t,
        raw_end: *mut bindings::nk_gpu_dev_coordinate_t,
        raw_pixel: *mut bindings::nk_gpu_dev_pixel_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensuers `raw_start` is a valid pointer.
        let start = unsafe { &*raw_start };

        // SAFETY: Caller ensuers `raw_end` is a valid pointer.
        let end = unsafe { &*raw_end };

        // SAFETY: Caller ensuers `raw_pixel` is a valid pointer.
        let pixel = unsafe { &*raw_pixel };

        G::graphics_draw_line(state, start, end, pixel).as_error_code()
    }

    unsafe extern "C" fn graphics_draw_poly(
        raw_state: *mut c_void,
        raw_coord_list: *mut bindings::nk_gpu_dev_coordinate_t,
        count: u32,
        raw_pixel: *mut bindings::nk_gpu_dev_pixel_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller (GPU subsystem) ensures `raw_coord_list` is a pointer to `count` coords.
        let coord_list = unsafe { core::slice::from_raw_parts_mut(raw_coord_list, count as usize) };

        // SAFETY: Caller ensuers `raw_pixel` is a valid pointer.
        let pixel = unsafe { &*raw_pixel };

        G::graphics_draw_poly(state, coord_list, pixel).as_error_code()
    }

    unsafe extern "C" fn graphics_fill_box_with_pixel(
        raw_state: *mut c_void,
        raw_rect: *mut bindings::nk_gpu_dev_box_t,
        raw_pixel: *mut bindings::nk_gpu_dev_pixel_t,
        op: bindings::nk_gpu_dev_bit_blit_op_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_rect` is a valid pointer.
        let rect = unsafe { &*raw_rect };

        // SAFETY: Caller ensuers `raw_pixel` is a valid pointer.
        let pixel = unsafe { &*raw_pixel };

        G::graphics_fill_box_with_pixel(state, rect, pixel, op).as_error_code()
    }

    unsafe extern "C" fn graphics_fill_box_with_bitmap(
        raw_state: *mut c_void,
        raw_rect: *mut bindings::nk_gpu_dev_box_t,
        raw_bitmap: *mut bindings::nk_gpu_dev_bitmap_t,
        op: bindings::nk_gpu_dev_bit_blit_op_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_rect` is a valid pointer.
        let rect = unsafe { &*raw_rect };

        // SAFETY: Caller ensures `raw_bitmap` is a valid pointer.
        let bitmap = unsafe { &*raw_bitmap };

        G::graphics_fill_box_with_bitmap(state, rect, bitmap, op).as_error_code()
    }

    unsafe extern "C" fn graphics_copy_box(
        raw_state: *mut c_void,
        raw_source_rect: *mut bindings::nk_gpu_dev_box_t,
        raw_dest_rect: *mut bindings::nk_gpu_dev_box_t,
        op: bindings::nk_gpu_dev_bit_blit_op_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_source_rect` is a valid pointer.
        let source_rect = unsafe { &*raw_source_rect };

        // SAFETY: Caller ensures `raw_dest_rect` is a valid pointer.
        let dest_rect = unsafe { &*raw_dest_rect };

        G::graphics_copy_box(state, source_rect, dest_rect, op).as_error_code()
    }

    unsafe extern "C" fn graphics_draw_text(
        raw_state: *mut c_void,
        raw_location: *mut bindings::nk_gpu_dev_coordinate_t,
        raw_font: *mut bindings::nk_gpu_dev_font_t,
        raw_text: *mut c_char,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_location` is a valid pointer.
        let location = unsafe { &*raw_location };

        // SAFETY: Caller ensures `raw_font` is a valid pointer.
        let font = unsafe { &*raw_font };

        // SAFETY: Caller ensures `raw_text` is a valid pointer.
        let c_str = unsafe { CStr::from_ptr(raw_text) };
        let text = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => {
                error!("graphics_draw_text called with non-UTF8 text.");
                return -1;
            }
        };

        G::graphics_draw_text(state, location, font, text).as_error_code()
    }

    unsafe extern "C" fn graphics_set_cursor_bitmap(
        raw_state: *mut c_void,
        raw_bitmap: *mut bindings::nk_gpu_dev_bitmap_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_bitmap` is a valid pointer.
        let bitmap = unsafe { &*raw_bitmap };

        G::graphics_set_cursor_bitmap(state, bitmap).as_error_code()
    }

    unsafe extern "C" fn graphics_set_cursor(
        raw_state: *mut c_void,
        raw_location: *mut bindings::nk_gpu_dev_coordinate_t,
    ) -> c_int {
        // SAFETY: On registration, `into_raw` was called, so it is safe to borrow from it here
        // because `from_raw` is called only after the device is unregistered.
        let state = unsafe { &*(raw_state as *const G::State) };

        // SAFETY: Caller ensures `raw_location` is a valid pointer.
        let location = unsafe { &*raw_location };

        G::graphics_set_cursor(state, location).as_error_code()
    }

    /// Registers a GPU device with Nautilus' GPU device subsytem.
    pub fn try_new(name: &str, data: Arc<G::State>) -> Result<Self> {
        let interface = Box::new(bindings::nk_gpu_dev_int {
            dev_int: bindings::nk_dev_int {
                open: None,
                close: None,
            },
            get_available_modes: Some(Registration::<G>::get_available_modes),
            get_mode: Some(Registration::<G>::get_mode),
            set_mode: Some(Registration::<G>::set_mode),
            flush: Some(Registration::<G>::flush),
            text_set_char: Some(Registration::<G>::text_set_char),
            text_set_cursor: Some(Registration::<G>::text_set_cursor),
            graphics_set_clipping_box: Some(Registration::<G>::graphics_set_clipping_box),
            graphics_set_clipping_region: Some(Registration::<G>::graphics_set_clipping_region),
            graphics_draw_pixel: Some(Registration::<G>::graphics_draw_pixel),
            graphics_draw_line: Some(Registration::<G>::graphics_draw_line),
            graphics_draw_poly: Some(Registration::<G>::graphics_draw_poly),
            graphics_fill_box_with_pixel: Some(Registration::<G>::graphics_fill_box_with_pixel),
            graphics_fill_box_with_bitmap: Some(Registration::<G>::graphics_fill_box_with_bitmap),
            graphics_copy_box: Some(Registration::<G>::graphics_copy_box),
            graphics_draw_text: Some(Registration::<G>::graphics_draw_text),
            graphics_set_cursor_bitmap: Some(Registration::<G>::graphics_set_cursor_bitmap),
            graphics_set_cursor: Some(Registration::<G>::graphics_set_cursor),
        });

        // Don't clean up this memory while the C code uses it.
        // We could have used `Arc` (and `into_raw`) here instead of a
        // `Box`, but the C code is the sole owner of the memory during
        // its lifetime.
        //
        // In theory, we also could have put `interface` in static
        // memory (since it is built of values known at compile-time),
        // but Rust does not have generic statics at the moment, and
        // we can't use `C` from the outer `impl` in that declaration.
        let interface_ptr = Box::into_raw(interface);

        // SAFETY: `name`, `interface_ptr`, and `data` are all valid pointers.
        // The call to `Box::from_raw` matches the call to `Box::into_raw` in the
        // error case.
        Ok(Self(unsafe {
            InternalRegistration::try_new(name, interface_ptr, data)
                .inspect_err(|_| { let _ = Box::from_raw(interface_ptr); })?
        }))
    }

    /// Gets the name of the GPU device.
    pub fn name(&self) -> &str {
        self.0.name.to_str().expect("Name cannot contain internal null bytes.")
    }

}

impl<G: GpuDev> Drop for Registration<G> {
    fn drop(&mut self) {
        let d = self.0.dev as *mut bindings::nk_gpu_dev;

        // SAFETY: Inside of `self.0.dev`, there is a pointer to the
        // GPU interface. This deallocation matches the call
        // to `Box::into_raw` in `Registration::try_new` in the success case.
        //
        // Note that we could have done this deallocation in `drop`
        // for `_InternalRegistration`, but this would technically
        // be dangerous if someone created an `_InternalRegistration`
        // without `Registration::try_new` (which no-one should ever do).
        // Anyway, it fits best here.
        let _ = unsafe { Box::from_raw((*d).dev.interface) };
    }
}
