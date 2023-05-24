use crate::prelude::*;
use crate::kernel::irq;

use super::lock::IRQLock;

mod virtio_gpu;

pub struct 

use crate::kernel::bindings;
use crate::kernel::utils::to_c_string;
use crate::prelude::*;
use super::lock::IRQLock;

use core::{
    ffi::{c_int, c_void, c_ushort},
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

pub struct NkGpuDev {
    dev: *mut bindings::nk_gpu_dev,
    name: String,
}

impl NkGpuDev {

}

impl Drop for NkGpuDev {
    fn drop(&mut self) {
        if let Some(ptr: &mut nk_gpu_dev) = unsafe { self.dev.as_mut() } {
            unsafe {
                let _ = Arc::from_raw(prt.dev.)
                bindings::nk_gpu_dev_unregister(ptr);
            }
        }
    }
}