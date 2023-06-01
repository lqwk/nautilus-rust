#![allow(unused_variables)]

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

#[derive(Copy, Clone)]
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

enum VirtioGpuFormat {
    B8G8R8A8Unorm  = 1 as _,
    B8G8R8X8Unorm  = 2 as _,
    A8R8G8B8Unorm  = 3 as _,
    X8R8G8B8Unorm  = 4 as _,
    R8G8B8A8Unorm  = 67 as _,
    X8B8G8R8Unorm  = 68 as _,
    A8B8G8R8Unorm  = 121 as _,
    R8G8B8X8Unorm  = 134 as _,
}

impl Default for VirtioGpuCtrlType {
    fn default() -> VirtioGpuCtrlType {
        VirtioGpuCtrlType::GetDisplayInfo
    }
}

#[derive(Default, Copy, Clone)]
struct CtrlHdr {
    type_: VirtioGpuCtrlType,
    flags: u32,
    fence_id: u64,
    ctx_id: u32,
    padding: u32,
}

#[derive(Default, Copy, Clone)]
struct GpuRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Default, Copy, Clone)]
struct DisplayOne {
    r: GpuRect,
    enabled: u32,
    flags: u32
}

#[derive(Default, Copy, Clone)]
struct RespDisplayInfo {
    hdr: CtrlHdr,
    pmodes: [DisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}

#[derive(Default)]
struct ResourceCreate2d {
    hdr: CtrlHdr,
    resource_id: u32,   // we need to supply the id, it cannot be zero
    format: u32,     // pixel format (as above)
    width: u32,          // resource size in pixels
    height: u32,         
}

#[derive(Default)]
struct ResourceAttachBacking {
    hdr: CtrlHdr,
    resource_id: u32,
    nr_entries: u32,
}

#[derive(Default)]
struct ResourceDetachBacking {
    hdr: CtrlHdr,
    resource_id: u32,
    padding: u32,
}

// #[derive(Default)]
struct MemEntry {
    addr: *const Box<[Pixel]>,
    length: u32,
    padding: u32,
}

#[derive(Default)]
struct SetScanout {
    hdr: CtrlHdr,
    r: GpuRect,
    scanout_id: u32,
    resource_id: u32,
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

        unsafe {
            transact_rw(
                &mut *self.virtio_dev,
                0,
                &[disp_info_req],
                &mut [self.disp_info_resp], // MATTHEW
            )?; // check ? operator
        }

        // CHECK RESP MACRO????

        for (i, mode) in self.disp_info_resp.pmodes.iter().enumerate() {
            if mode.enabled != 0 {
                vc_println!("scanout (monitor) {} has info: x={}, y={}, {} by {} flags=0x{} enabled={}",
                    i,
                    mode.r.x,
                    mode.r.y,
                    mode.r.width,
                    mode.r.height,
                    mode.flags,
                    mode.enabled);
            }
        }
        
        self.have_disp_info = true;

        Ok(())
    }

    fn reset(&mut self) -> Result {
        unimplemented!();
    }
}

type State = IRQLock<VirtioGpuDev>;
unsafe impl Send for VirtioGpuDev {}

extern "C" {
    fn _glue_vga_copy_out(dest: *mut u16, len: usize);
    fn _glue_vga_copy_in(src: *mut u16, len: usize);
}

impl gpudev::GpuDev for VirtioGpuDev {

    type State = State;

    fn get_available_modes(state: &Self::State, modes: &mut [VideoMode]) -> Result<usize> { 
        // let state = state.lock();


        if modes.len() < 2 {
            error!("Must provide at least two mode slots\n");
            return Err(-1);
        }
     
        if state.lock().update_modes().is_err() {
            error!("Cannot update modes\n");
            return Err(-1);

        }
        // now translate modes back to that expected by the abstraction
        // we will interpret each scanout as a mode, plus add a text mode as well
        let limit = if modes.len() > 16 { 15 } else { modes.len() - 1 };
        let mut cur: usize = 0;

        
        modes[cur] = state.lock().gen_mode(0);
        cur += 1;

        // graphics modes
        for i in 0..16 {
            if cur < limit {
                break;
            }
            modes[cur] = state.lock().gen_mode(i+1);
            cur += 1;
        }

        Ok(cur)
    }

    fn get_mode(state: &Self::State) -> Result<VideoMode> {

        let state = state.lock();
        Ok(state.gen_mode(state.cur_mode))
    }
    
    

    // set a video mode based on the modes discovered
    // this will switch to the mode before returning
    fn set_mode(
        state: &Self::State, 
        mode: &VideoMode
    ) -> Result {
        // let state = state.lock();
        let mode_num = mode.mode_data as usize;

        info!("set mode on virtio-gpu0"); // can we access name from InternalReg

        if state.lock().cur_mode == 0 {
            unsafe { _glue_vga_copy_out(state.lock().text_snapshot.as_ptr() as _, 80 * 25 * 2); } // needs to be mutable?
            info!("copy out of text mode data complete");
        }

        if state.lock().reset().is_err() {
            error!("Cannot reset device");
            return Err(-1);
        } 

        info!("reset complete");
        if mode_num == 0 {
            unsafe {_glue_vga_copy_in(state.lock().text_snapshot.as_ptr() as _, 80 * 25 * 2); }
            info!("copy in of text mode data complete");
            info!("switch to text mode complete");
            return Ok(());
        }

        let pm = state.lock().disp_info_resp.pmodes[mode_num - 1];
        let create_2d_req = ResourceCreate2d {
            hdr: CtrlHdr {
                type_: VirtioGpuCtrlType::ResourceCreate2D,
                ..Default::default()
            },
            resource_id: 42, // SCREEN_RID
            format: VirtioGpuFormat::R8G8B8A8Unorm as u32,
            width: pm.r.width,
            height: pm.r.height,
        };
        let create_2d_resp = CtrlHdr::default();

        info!("doing transaction to create 2D screen");

        unsafe {
            transact_rw(
                &mut *state.lock().virtio_dev,
                0,
                &[create_2d_req],
                &mut [create_2d_resp],
            ).expect("failed to create 2D screen (transaction failed"); // will propogate error?
        }

        // CHECK_RESP?

        info!("transaction complete");

        // 3. we would create a framebuffer that we can write pixels into
        let fb_len: usize = (pm.r.width * pm.r.height * core::mem::size_of::<Pixel>() as u32) as usize;
        let mut frame_buffer = Box::new([Pixel::default()]);
        state.lock().frame_buffer = Some(frame_buffer);
        info!("allocated screen framebuffer of length {}", fb_len); // may not need fb_len at all, unless for debug

        // now create a description of it in a bounding box
        state.lock().frame_box = Rect {
            x: 0,
            y: 0,
            width: pm.r.width,
            height: pm.r.height,
        };

        // make the clipping box the entire screen
        state.lock().clipping_box = Rect {
            x: 0,
            y: 0,
            width: pm.r.width,
            height: pm.r.height,
        };

        // 4. we should probably fill the framebuffer with some initial data
        // A typical driver would fill it with zeros (black screen), but we
        // might want to put something more exciting there.

        info!("filling framebuffer with initial screen");

        // 5. Now we need to associate our framebuffer (step 4) with our resource (step 2)

        let backing_req = ResourceAttachBacking {
            hdr: CtrlHdr {
                type_: VirtioGpuCtrlType::ResourceAttachBacking,
                ..Default::default()
            },
            resource_id: 42, // SCREEN_RID
            nr_entries: 1,
        };
        let backing_entry = MemEntry {
            addr: state.lock().frame_buffer.as_ref().unwrap(),
            length: fb_len as u32,
            padding: 0,
        };
        let backing_resp = CtrlHdr::default();

        info!("doing transaction to associate framebuffer with screen resource");

        if unsafe {
            transact_rrw(
                state.lock().virtio_dev,
                0,
                &[backing_req],
                &[backing_entry],
                &mut [backing_resp],
            ).is_err()
        } {
            error!("failed to associate framebuffer with screen resource (transaction failed)");
            return Err(-1);
        }
        // CHECK_RESP?
        info!("transaction complete");

        // 6. Now we need to associate our resource (step 2) with the scanout (step 1)
        //    use mode_num-1 as the scanout ID

        let setso_req = SetScanout {
            hdr: CtrlHdr {
                type_: VirtioGpuCtrlType::SetScanout,
                ..Default::default()
            },
            r: pm.r,
            scanout_id: mode_num as u32 - 1,
            resource_id: 42, // SCREEN_RID
        };
        let setso_resp = CtrlHdr::default();

        info!("doing transaction to associate screen resource with the scanout");
        if unsafe {
            transact_rw(
                &mut *state.lock().virtio_dev,
                0,
                &[setso_req],
                &mut [setso_resp],
            ).is_err()
        } {
            error!("failed to associate screen resource with the scanout (transaction failed)");
            return Err(-1);
        }
        // CHECK_RESP?
        info!("transaction complete");

        // Now let's capture our mode number to indicate we are done with setup
        // and make subsequent calls aware of our state
        state.lock().cur_mode = mode_num;
        // Self::flush(state)?;


        Ok(())
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
            vc_println!("Failed to free descriptor chain");
            return Err(-1);
        }
    }

    Ok(())
}

unsafe fn transact_rw<T1, T2>(
    dev: &mut bindings::virtio_pci_dev,
    qidx: u16,
    req: &[T1],
    resp: &mut [T2]
) -> Result {
    let mut desc_idx = [0_u16; 2];
    let reqlen = (core::mem::size_of::<T1>() * req.len()) as u32;
    let resplen = (core::mem::size_of::<T2>() * resp.len()) as u32;

    unsafe {
        // allocate a two element descriptor chain, the descriptor
        // numbers will be placed in the desc_idx array.
        if bindings::virtio_pci_desc_chain_alloc(dev as *mut _, qidx, desc_idx.as_mut_ptr(), 2) != 0 {
            vc_println!("Failed to allocate descriptor chain");
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

    unsafe { transact_base(dev, qidx, desc_idx[0]) }
    // Ok(())
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
        vc_println!("Could not acknowledge device");
        return -1;
    }

    // Ask the device for what features it supports
    if unsafe { bindings::virtio_pci_read_features(virtio_dev) } != 0 {
        vc_println!("Unable to read device features");
        return -1;
    }

    // Tell the device what features we will support.
    //
    // We will not support either VIRGL (3D) or EDID (better display info) for now.
    if unsafe { bindings::virtio_pci_write_features(virtio_dev, 0) } != 0 {
        vc_println!("Unable to write device features");
        return -1;
    }

    // Initilize the device's virtqs. The virtio-gpu device
    // has two of them.  The first is for most requests/responses,
    // while the second is for (mouse) cursor updates and movement
    if unsafe { bindings::virtio_pci_virtqueue_init(virtio_dev) } != 0 {
        vc_println!("failed to initialize virtqueues");
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
