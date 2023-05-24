use crate::prelude::*;
use crate::kernel::irq;



use crate::kernel::bindings;
use crate::kernel::utils::to_c_string;
use crate::prelude::*;
// use super::lock::IRQLock;

use core::{
    ffi::{c_int, c_void, c_ushort,},
    intrinsics::write_bytes,
    ptr::null_mut,
};

use alloc::{borrow::ToOwned, string::String, sync::Arc};

// pub struct VirtioGpuDev {
//     gpu_dev: *mut bindings::nk_gpu_dev,
//     virtio_dev: *mut bindings::virtio_pci_dev,
//     spinlock: UnsafeCell<bindings::spinlock_t>,
//     have_disp_info: c_int,
//     disp_info_resp: bindings::virtio_gpu_config_display_info,
//     cur_mode: c_int,
//     frame_buffer: *mut c_void,
//     frame_box: bindings::nk_gpu_dev_box_t,
//     clipping_box: bindings::nk_gpu_dev_box_t,
//     cursor_buffer: *mut c_void,
//     cursor_box: bindings::nk_gpu_dev_box_t,
//     text_snapshot: c_ushort,
// }

// impl VirtioGpuDev {

// }

// Not sure if necessary at all?
pub struct GpuDevMode {
    width: u32,
    height: u32,
    channel_offset: [u32; 4],
    flags: u32,
    mouse_cursor_width: u32,
    mouse_cursor_height: u32,
    mode_data: c_void,
}

// Not sure if necessary at all?
pub enum VideoModes {
    ChannelOffsetRed,
    ChannelOffsetGreen,
    ChannelOffsetBlue,
    ChannelOffsetAlpha,
    ChannelOffsetText,
    ChannelOffsetAttr,
    HasClipping,
    HasClippingRegion,
    HasMouseCursor,
}

impl core::convert::From<VideoModes> for c_int {
    fn from(mode: VideoModes) -> Self {
        match mode {
            VideoModes::ChannelOffsetRed => 0,
            VideoModes::ChannelOffsetGreen => 1,
            VideoModes::ChannelOffsetBlue => 2,
            VideoModes::ChannelOffsetAlpha => 3,
            VideoModes::ChannelOffsetText => 0,
            VideoModes::ChannelOffsetAttr => 1,
            VideoModes::HasClipping => 0x1,
            VideoModes::HasClippingRegion => 0x2,
            VideoModes::HasMouseCursor => 0x100,
        }
    }
}

// Not sure if this is the right track 
pub enum PassFail<T = ()> {
    Ok(T),
    Err,
}

impl<T> core::convert::From<PassFail<T>> for c_int {
    fn from (pf: PassFail<T>) -> Self {
        match pf {
            PassFail::Ok(_) => 0,
            PassFail::Err => -1,
        }
    }
}

pub trait GpuDev {
    type State: Send + Sync;

    fn get_available_modes(
        state: &Self::State, 
        // modes: Vec<GpuDevMode>,
        // num_modes: &u32
    ) -> PassFail;

    fn set_mode(
        state: &Self::State, 
        // mode: *mut GpuDevMode
    ) -> PassFail;

    fn get_mode(
        state: &Self::State, 
        // mode: *mut GpuDevMode
    ) -> PassFail;


    // All should be async but not supported in trait impl
    fn flush(state: &Self::State) -> PassFail;

    fn text_set_char(
        state: &Self::State,
        // location: &bindings::nk_gpu_dev_coordinate_t,
        // val: &bindings::nk_gpu_dev_char_t,
    ) -> PassFail;

    fn text_set_cursor(
        state: &Self::State,
        // location: &bindings::nk_gpu_dev_coordinate_t,
        // flags: &u32/
    ) -> PassFail;

    fn graphics_set_clipping_box(
        state: &Self::State,
        // box_: &bindings::nk_gpu_dev_box_t,
    ) -> PassFail;
    
    fn graphics_set_clipping_region(
        state: &Self::State,
        // region: &bindings::nk_gpu_dev_region_t,
    ) -> PassFail;
    
    fn graphics_draw_pixel (
        state: &Self::State,
        // start: &bindings::nk_gpu_dev_coordinate_t,
        // end: &bindings::nk_gpu_dev_coordinate_t,
        // pixel: &bindings::nk_gpu_dev_pixel_t,
    ) -> PassFail;

    fn graphics_draw_line(
        state: &Self::State,
        // start: &bindings::nk_gpu_dev_coordinate_t,
        // end: &bindings::nk_gpu_dev_coordinate_t,
        // pixel: &bindings::nk_gpu_dev_pixel_t,
    ) -> PassFail;

    fn graphics_draw_poly(
        state: &Self::State,
        // coord_list: &bindings::nk_gpu_dev_coordinate_t,
        // count: u32,
        // pixel: &bindings::nk_gpu_dev_pixel_t,
    ) -> PassFail;

    fn graphics_fill_box_with_pixel(
        state: &Self::State,
        // _box: &bindings::nk_gpu_dev_box_t,
        // pixel: &bindings::nk_gpu_dev_pixel_t,
        // op: bindings::nk_gpu_dev_bit_blit_op_t,
    ) -> PassFail;

    fn graphics_fill_box_with_bitmap(
        state: &Self::State,
        // _box: &bindings::nk_gpu_dev_box_t,
        // bitmap: &bindings::nk_gpu_dev_bitmap_t,
        // op: &bindings::nk_gpu_dev_bit_blit_op_t,
    ) -> PassFail;

    fn graphics_copy_box(
        state: &Self::State,
        // source_box: &bindings::nk_gpu_dev_box_t,
        // dest_box: &bindings::nk_gpu_dev_box_t, 
        // op: &bindings::nk_gpu_dev_bit_blit_op_t,
    ) -> PassFail;

    fn graphics_draw_text(
        state: &Self::State,
        // location: &bindings::nk_gpu_dev_coordinate_t,
        // font: &bindings::nk_gpu_dev_font_t,
        // string: c_string,
    ) -> PassFail;

    fn graphics_set_cursor_bitmap (
        state: &Self::State,
        // bitmap: &bindings::nk_gpu_dev_bitmap_t,
    ) -> PassFail;

    fn graphics_set_cursor(
        state: &Self::State,
        // location: &bindings:nk_gpu_dev_coordinate_t,
    ) -> PassFail;

}

// only here so rust-analyzer will function, otherwise cant check code in the impl
pub struct Registration<G: GpuDev>;

impl<G: GpuDev>Registration<G> {
    async unsafe extern "C" fn get_available_modes(
        raw_state: c_void, 
        modes: *mut bindings::nk_gpu_dev_video_mode_t,
        num_modes: *mut u32
    ) -> c_int {
        let state = unsafe { (raw_state as *const G::State).as_ref() }.unwrap();

        if num_modes < 2 {
            PassFail::Err.into()
        }
    }

    unsafe extern "C" fn set_mode(
        raw_state: c_void, 
        mode: *mut bindings::nk_gpu_dev_video_mode_t,
    ) -> c_int {
        let state = unsafe { (raw_state as *const G::State).as_ref() }.unwrap();
    }
}