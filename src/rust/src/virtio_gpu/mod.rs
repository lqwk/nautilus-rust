use core::ffi::{c_void, c_int};

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

impl Default for VirtioGpuCtrlType {
    fn default() -> VirtioGpuCtrlType {
        VirtioGpuCtrlType::GetDisplayInfo
    }
}

#[derive(Default)]
struct CtrlHdr {
    type_: VirtioGpuCtrlType,
    flags: u32,
    fence_id: u64,
    ctx_id: u32,
    padding: u32,
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
    cur_mode: usize,
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
impl VirtioGpuDev {
    fn gen_mode(&self, modenum: usize) -> VideoMode{
        if modenum == 0 {
            VideoMode {
                type_: VideoModeType::Text as _,
                width: 80,
                height: 25,
                channel_offset: [0, 1, 0xFF, 0xFF],
                flags: 0,
                mouse_cursor_width: 0,
                mouse_cursor_height: 0,
                mode_data: modenum as *mut c_void
            }
        } else {
            VideoMode {
                type_: VideoModeType::Graphics as _,
                width: self.disp_info_resp.pmodes[modenum - 1].r.width,
                height: self.disp_info_resp.pmodes[modenum - 1].r.height,
                channel_offset: [0, 1, 2, 3],
                flags: bindings::NK_GPU_DEV_HAS_MOUSE_CURSOR as _,
                mouse_cursor_width: 64,
                mouse_cursor_height: 64,
                mode_data: modenum as *mut c_void
            }
        }
    }

    fn update_modes(&mut self) -> Result {
        if self.have_disp_info {
            return Ok(());
        }

        let mut disp_info_req = CtrlHdr::default();
        self.disp_info_resp = RespDisplayInfo::default();

        disp_info_req.type_ = VirtioGpuCtrlType::GetDisplayInfo;

        Ok(())
    }
}

type State = IRQLock<VirtioGpuDev>;
unsafe impl Send for VirtioGpuDev {}

impl gpudev::GpuDev for VirtioGpuDev {

    type State = State;

    fn get_available_modes(state: &Self::State, modes: &mut [VideoMode]) -> Result<usize> {
        debug!("get_available_modes");
         
        if modes.len() < 2 {
            error!("Must provide at least two mode slots.");
            return Err(-1);
        }

        unimplemented!();
    }

    fn get_mode(state: &Self::State) -> Result<VideoMode> {
        debug!("get_mode");

        let state = state.lock();
        Ok(state.gen_mode(state.cur_mode))
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

extern "C" {
    fn _glue_mbarrier();
    fn _glue_virtio_pci_atomic_store_u16(destptr: *mut u16, value: u16);
    fn _glue_virtio_pci_atomic_load_u16(srcptr: *mut u16) -> u16;
}

unsafe fn transact_base(dev: &mut bindings::virtio_pci_dev, qidx: u16, didx: u16) -> Result {
    let virtq = &mut (dev.virtq[qidx as usize]);
    let vq = &mut (virtq.vq);

    unsafe {
        // the following steps push didx onto the virtqueue
        // in a manner acceptable to the hardware
        (*vq.avail).ring.as_mut_ptr().offset(((*vq.avail).idx % vq.qsz) as isize).write(didx);
        // this memory barrier makes sure the device sees
        // the above write *before*...
        _glue_mbarrier();
        // ... this write:
        (*vq.avail).idx += 1;
        // we will stash away the index in the used ring
        // which we will wait on
        let waitidx = (*vq.avail).idx;
        // and memory barrier again to be sure these
        // two writes are globally visible
        _glue_mbarrier(); 
        // Now we are going to notify the device
        // The device's registers are memory mapped, meaning that
        // the structure read/writes below are going all the way
        // to the device

        // select the virtqueue we want to notify
        _glue_virtio_pci_atomic_store_u16(&mut (*dev.common).queue_select as *mut _, qidx);

        // make sure it is running
        _glue_virtio_pci_atomic_store_u16(&mut (*dev.common).queue_enable as *mut _, 1);

        // ask the virtio-pci subsystem we live in to actually do the
        // notification write
        bindings::virtio_pci_virtqueue_notify(dev as *mut _, qidx);

        // The device has our request now

        // Satisfy the borrow checker by shadowing the old borrow
        let virtq = &mut (dev.virtq[qidx as usize]);

        // wait for the hardware to complete our request and
        // move it to the used ring
        // Ideally we would not do this dumb polling here, but
        // make everything interrupt driven.
        let mut usedidx = _glue_virtio_pci_atomic_load_u16(&mut (*virtq.vq.used).idx as *mut _);
        while usedidx != waitidx {
          usedidx = _glue_virtio_pci_atomic_load_u16(&mut (*virtq.vq.used).idx as *mut _);
        }

        // now we are done with the descriptor chain, so ask
        // the virtio-pci system to clean it up for us
        if bindings::virtio_pci_desc_chain_free(dev as *mut _,qidx,didx) != 0 {
            error!("Failed to free descriptor chain");
            return Err(-1);
        }
    }

    Ok(())
}

unsafe fn transact_rw<T>(
    dev: &mut bindings::virtio_pci_dev,
    qidx: u16,
    req: &[T],
    resp: &mut [CtrlHdr]
) -> Result {
    let mut desc_idx = [0_u16; 2];
    let reqlen = (core::mem::size_of::<T>() * req.len()) as u32;
    let resplen = (core::mem::size_of::<CtrlHdr>() * resp.len()) as u32;

    unsafe {
        // allocate a two element descriptor chain, the descriptor
        // numbers will be placed in the desc_idx array.
        if bindings::virtio_pci_desc_chain_alloc(dev as *mut _, qidx, desc_idx.as_mut_ptr(), 2) != 0 {
            error!("Failed to allocate descriptor chain");
            return Err(-1);
        }

        // Now get pointers to the specific descriptors in the virtq struct
        // (which is shared with the hardware)
        let desc = [dev.virtq[qidx as usize].vq.desc.offset(desc_idx[0] as isize),
                    dev.virtq[qidx as usize].vq.desc.offset(desc_idx[1] as isize)];

        // now build a linked list of 2 elements in this space

        // this is the "read" part - the request
        // first element of the linked list
        (*desc[0]).addr = req.as_ptr() as u64;
        (*desc[0]).len = reqlen;
        (*desc[0]).flags |= 0;
        (*desc[0]).next = desc_idx[1];  // next pointer is next descriptor
                                        //
        // this is the "write" part - the response
        // this is where we want the device to put the response
        (*desc[1]).addr = resp.as_ptr() as u64;
        (*desc[1]).len = resplen;
        (*desc[1]).flags |= bindings::VIRTQ_DESC_F_WRITE as u16;
        (*desc[1]).next = 0;            // next pointer is null   
    }

    Ok(())
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
