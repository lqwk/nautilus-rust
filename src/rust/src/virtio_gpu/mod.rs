use crate::prelude::*;

use crate::kernel::{
    bindings, 
    gpudev,
    gpudev::{VideoMode, Char, Coordinate, Font, Rect, BitBlitOp, Pixel, Region, Bitmap},
    print::make_logging_macros,
    sync::IRQLock,
};

make_logging_macros!("virtio_gpu", NAUT_CONFIG_DEBUG_VIRTIO_GPU);

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

enum VirtioGpuCtrlType {
    GetDisplayInfo = 0x0100,
    ResourceCreate2D,
    ResourceUnref,
    SetScanout,
    ResourceFlush,
    TransferToHost2D,
    ResourceAttachBacking,
    ResourceDetachBacking,
    GetCapsetInfo,
    GetCapset,
    GetEdid,
    UpdateCursor = 0x0300,
    MoveCursor,
    OkNoData = 0x1100,
    OkDisplayInfo,
    OkCapsetInfo,
    OkCapset,
    OkEdid,
    ErrUnspec = 0x1200,
    ErrOutOfMemory,
    ErrInvalidScanoutId,
    ErrInvalidResourceId,
    ErrInvalidContextId,
    ErrInvalidParameter,
}

struct CtrlHdr {
    type_: VirtioGpuCtrlType,
    flags: u32,
    fence_id: u64,
    ctx_id: u32,
    padding: u32,
}

impl Default for CtrlHdr {
    fn default() -> CtrlHdr {
        CtrlHdr { // C code memsets 0...may be better to use '0 as *mut ...' instead
            type_: VirtioGpuCtrlType::GetDisplayInfo,
            flags: 0,
            fence_id: 0,
            ctx_id: 0,
            padding: 0,
        }
    }
}

#[derive(Default)]
struct GpuRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Default)]
struct DisplayOne {
    r: GpuRect,
    enabled: u32,
    flags: u32
}

#[derive(Default)]
struct RespDisplayInfo {
    hdr: CtrlHdr,
    pmodes: [DisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}


struct VirtioGpuDev {
    gpu_dev: Option<gpudev::Registration<Self>>,
    virtio_dev: *mut bindings::virtio_pci_dev,
    have_disp_info: bool,
    disp_info_resp: RespDisplayInfo,
    cur_mode: u32,
    frame_buffer: Option<Box<[Pixel]>>,
    frame_box: Rect,
    clipping_box: Rect,
    text_snapshot: [u16; 80 * 25],
}

impl Default for VirtioGpuDev {
    fn default() -> VirtioGpuDev {
        VirtioGpuDev {
            gpu_dev: None,
            virtio_dev: core::ptr::null_mut(),
            have_disp_info: false,
            disp_info_resp: RespDisplayInfo::default(),
            cur_mode: 0,
            frame_buffer: None,
            frame_box: Rect::default(),
            clipping_box: Rect::default(),
            text_snapshot: [0; 80 * 25],
        }
    }
}


#[repr(i32)] 
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VideoModeType {
    Text = bindings::nk_gpu_dev_video_mode_NK_GPU_DEV_MODE_TYPE_TEXT as _,
    Graphics = bindings::nk_gpu_dev_video_mode_NK_GPU_DEV_MODE_TYPE_GRAPHICS_2D as _,
}

// gpu device-specific functions
impl VirtioGpuDev { }

type State = IRQLock<VirtioGpuDev>;
unsafe impl Send for VirtioGpuDev {}

impl gpudev::GpuDev for VirtioGpuDev {

    type State = State;

    fn get_available_modes(state: &Self::State, modes: &mut [VideoMode]) -> Result<usize> {
        unimplemented!();
    }

    fn get_mode(state: &Self::State) -> Result<VideoMode> {
        unimplemented!();
    }
    
    // set a video mode based on the modes discovered
    // this will switch to the mode before returning
    fn set_mode(
        state: &Self::State, 
        mode: &VideoMode
    ) -> Result {
        unimplemented!();
    }

    // drawing commands
    
    // each of these is asynchronous - the implementation should start the operation
    // but not necessarily finish it.   In particular, nothing needs to be drawn
    // until flush is invoked

    // flush - wait until all preceding drawing commands are visible by the user
    fn flush(state: &Self::State) -> Result {
        unimplemented!();
    }

    // text mode drawing commands
    fn text_set_char(state: &Self::State, location: &Coordinate, val: &Char) -> Result {
        unimplemented!();
    }

    // cursor location in text mode
    fn text_set_cursor(state: &Self::State, location: &Coordinate, flags: u32) -> Result {
        unimplemented!();
    }

    // graphics mode drawing commands
    // confine drawing to this box or region
    fn graphics_set_clipping_box(state: &Self::State, rect: &Rect) -> Result {
        unimplemented!();
    }

    fn graphics_set_clipping_region(state: &Self::State, region: &Region) -> Result {
        unimplemented!();
    }

    // draw stuff 
    fn graphics_draw_pixel(
        state: &Self::State, 
        location: &Coordinate, 
        pixel: &Pixel
    ) -> Result {
        unimplemented!();
    }
    fn graphics_draw_line(
        state: &Self::State, 
        start: &Coordinate, 
        end: &Coordinate, 
        pixel: &Pixel
    ) -> Result {
        unimplemented!();
    }
    fn graphics_draw_poly(
        state: &Self::State, 
        coord_list: &[Coordinate], 
        pixel: &Pixel
    ) -> Result {
        unimplemented!();
    }
    fn graphics_fill_box_with_pixel(
        state: &Self::State, 
        rect: &Rect, 
        pixel: &Pixel, 
        op: BitBlitOp
    ) -> Result {
        unimplemented!();
    }
    fn graphics_fill_box_with_bitmap(
        state: &Self::State, 
        rect: &Rect, 
        bitmap: &Bitmap, 
        op: BitBlitOp
    ) -> Result {
        unimplemented!();
    }
    fn graphics_copy_box(
        state: &Self::State, 
        source_rect: &Rect, 
        dest_box: &Rect, 
        op: BitBlitOp
    ) -> Result {
        unimplemented!();
    }
    fn graphics_draw_text(
        state: &Self::State, 
        location: &Coordinate, 
        font: &Font, text: &str
    ) -> Result {
        unimplemented!();
    }

    // mouse functions, if supported
    fn graphics_set_cursor_bitmap(state: &Self::State, bitmap: &Bitmap) -> Result {
        unimplemented!();
    }
    // the location is the position of the top-left pixel in the bitmap
    fn graphics_set_cursor(state: &Self::State, location: &Coordinate) -> Result {
        unimplemented!();
    }
}

fn transact_base(
    dev: *mut bindings::virtio_pci_dev,
    qidx: u16,
    didx: u16,
) -> Result {
    unimplemented!();
}

fn transact_rw<T>(
    dev: *mut bindings::virtio_pci_dev,
    qidx: u16,
    req: &[T],
    resp: &mut [CtrlHdr]
) -> Result {
    unimplemented!();
}

fn transact_rrw<R1, R2>(
    dev: *mut bindings::virtio_pci_dev,
    qidx: u16,
    req: &[R1],
    more: &[R2],
    resp: &mut [CtrlHdr]
) -> Result {
    unimplemented!();
}

#[no_mangle]
extern "C" fn virtio_gpu_init(virtio_dev: *mut bindings::virtio_pci_dev) -> core::ffi::c_int {
    info!("init");

    // Allocate a default state structure for this device
    let dev = Arc::new(IRQLock::new(VirtioGpuDev::default()));

    // Acknowledge to the device that we see it
    if unsafe { bindings::virtio_pci_ack_device(virtio_dev) } != 0 {
        error!("Could not acknowledge device");
        return -1;
    }

    // Ask the device for what features it supports
    if unsafe { bindings::virtio_pci_read_features(virtio_dev) } != 0 {
        error!("Unable to read device features");
        return -1;
    }

    // Tell the device what features we will support.
    //
    // We will not support either VIRGL (3D) or EDID (better display info) for now.
    if unsafe { bindings::virtio_pci_write_features(virtio_dev, 0) } != 0 {
        error!("Unable to write device features");
        return -1;
    }

    // Initilize the device's virtqs. The virtio-gpu device
    // has two of them.  The first is for most requests/responses,
    // while the second is for (mouse) cursor updates and movement
    if unsafe { bindings::virtio_pci_virtqueue_init(virtio_dev) } != 0 {
        error!("failed to initialize virtqueues");
        return -1;
    }

    // Associate our state with the general virtio-pci device structure,
    // and vice-versa:
    let dev_ptr = &*dev.lock() as *const _ as *mut VirtioGpuDev;
    unsafe {
        (*virtio_dev).state = dev_ptr as *mut _;
        (*virtio_dev).teardown = None;
        (*dev_ptr).virtio_dev = virtio_dev;
    }

    // Register the GPU device. We will only support the first Virtio GPU device
    // (virtio-gpu0).
    let res = gpudev::Registration::<VirtioGpuDev>::try_new("virtio-gpu0", Arc::clone(&dev));
    match res {
        Ok(registration) => {
            unsafe { (*dev_ptr).gpu_dev = Some(registration); }
        },
        Err(e) => {
            return e;
        }
    }

    // Could enable interrupts for the device here, but it's not necessary for this
    // simple driver.

    return 0;

}
