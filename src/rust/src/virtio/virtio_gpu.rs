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

use crate::prelude::*;
use crate::kernel::irq;

use super::lock::IRQLock;

mod virtio_gpu;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum VirtioGPUCtrlType {
    // 2d commands
    VIRTIO_GPU_CMD_GET_DISPLAY_INFO = 0x0100,
    VIRTIO_GPU_CMD_RESOURCE_CREATE_2D,
    VIRTIO_GPU_CMD_RESOURCE_UNREF,

    VIRTIO_GPU_CMD_SET_SCANOUT,
    VIRTIO_GPU_CMD_RESOURCE_FLUSH,
    VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D,
    VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING, 
    VIRTIO_GPU_CMD_RESOURCE_DETACH_BACKING,
    VIRTIO_GPU_CMD_GET_CAPSET_INFO,
    VIRTIO_GPU_CMD_GET_CAPSET,
    VIRTIO_GPU_CMD_GET_EDID,

    // cursor commands
    VIRTIO_GPU_CMD_UPDATE_CURSOR = 0x0300,
    VIRTIO_GPU_CMD_MOVE_CURSOR,

    // success responses
    VIRTIO_GPU_RESP_OK_NODATA = 0x1100,
    VIRTIO_GPU_RESP_OK_DISPLAY_INFO,
    VIRTIO_GPU_RESP_OK_CAPSET_INFO,
    VIRTIO_GPU_RESP_OK_CAPSET,
    VIRTIO_GPU_RESP_OK_EDID,

    // error responses
    VIRTIO_GPU_RESP_ERR_UNSPEC = 0x1200,
    VIRTIO_GPU_RESP_ERR_OUT_OF_MEMORY,
    VIRTIO_GPU_RESP_ERR_INVALID_SCANOUT_ID,
    VIRTIO_GPU_RESP_ERR_INVALID_RESOURCE_ID,
    VIRTIO_GPU_RESP_ERR_INVALID_CONTEXT_ID,
    VIRTIO_GPU_RESP_ERR_INVALID_PARAMETER,
}