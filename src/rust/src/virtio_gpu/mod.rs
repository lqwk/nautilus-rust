#![allow(unused_variables)]

use core::ffi::{c_void, c_int};
use core::cell::RefCell;

use crate::prelude::*;
use crate::kernel::{
    bindings, 
    gpudev,
    gpudev::{VideoMode, Char, Coordinate, Font, Rect, BitBlitOp, Pixel, Region, Bitmap},
    print::make_logging_macros,
};

make_logging_macros!("virtio_gpu", NAUT_CONFIG_DEBUG_VIRTIO_GPU);

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;
const SCREEN_RID: u32 = 42;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(i32)]
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

#[repr(u32)]
enum VirtioGpuFormat {
    B8G8R8A8Unorm  = 1,
    B8G8R8X8Unorm  = 2,
    A8R8G8B8Unorm  = 3,
    X8R8G8B8Unorm  = 4,
    R8G8B8A8Unorm  = 67,
    X8B8G8R8Unorm  = 68,
    A8B8G8R8Unorm  = 121,
    R8G8B8X8Unorm  = 134,
}

impl Default for VirtioGpuCtrlType {
    fn default() -> VirtioGpuCtrlType {
        VirtioGpuCtrlType::GetDisplayInfo
    }
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
struct CtrlHdr {
    type_: VirtioGpuCtrlType,
    flags: u32,
    fence_id: u64,
    ctx_id: u32,
    padding: u32,
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
struct GpuRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
struct DisplayOne {
    r: GpuRect,
    enabled: u32,
    flags: u32
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
struct RespDisplayInfo {
    hdr: CtrlHdr,
    pmodes: [DisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}

#[derive(Default)]
#[repr(C)]
struct ResourceCreate2d {
    hdr: CtrlHdr,
    resource_id: u32,   // we need to supply the id, it cannot be zero
    format: u32,     // pixel format (as above)
    width: u32,          // resource size in pixels
    height: u32,         
}

#[derive(Default)]
#[repr(C)]
struct ResourceAttachBacking {
    hdr: CtrlHdr,
    resource_id: u32,
    nr_entries: u32,
}

#[derive(Default)]
#[repr(C)]
struct ResourceDetachBacking {
    hdr: CtrlHdr,
    resource_id: u32,
    padding: u32,
}

#[derive(Default)]
#[repr(C)]
struct MemEntry {
    addr: u64,
    length: u32,
    padding: u32,
}

#[derive(Default)]
#[repr(C)]
struct SetScanout {
    hdr: CtrlHdr,
    r: GpuRect,
    scanout_id: u32,
    resource_id: u32,
}

#[derive(Default)]
#[repr(C)]
struct TransferToHost2D {
    hdr: CtrlHdr,
    r: GpuRect,
    offset: u64,
    resource_id: u32,
    padding: u32
}

#[derive(Default)]
#[repr(C)]
struct ResourceFlush {
    hdr: CtrlHdr,
    r: GpuRect,
    resource_id: u32,
    padding: u32
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

fn get_bitmap_pixel(bitmap: &Bitmap, x: u32, y: u32) -> Option<&'_ Pixel>{
    if x >= bitmap.width || y >= bitmap.height {
        None
    } else {
        unsafe { Some(&bitmap
                      .pixels
                      .as_slice((bitmap.width * bitmap.height) as usize)[(x + y * (bitmap.width)) as usize]) }
    }
}

// gpu device-specific functions
impl VirtioGpuDev {
    fn name(&self) -> &'_ str {
        self.gpu_dev.as_ref().unwrap().name()
    }

    fn get_pixel(&mut self, x: u32, y: u32) -> &'_ mut Pixel {
        &mut self.frame_buffer.as_mut().unwrap()[(y * self.frame_box.width + x) as usize]
    }

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
            transact_rw(&mut *self.virtio_dev, 0, &disp_info_req, &mut self.disp_info_resp)?;
        }

        check_response(&self.disp_info_resp.hdr, VirtioGpuCtrlType::OkDisplayInfo, "Failed to get display info")?;

        for (i, mode) in self.disp_info_resp.pmodes.iter().enumerate() {
            if mode.enabled != 0 {
                debug!("scanout (monitor) {} has info: x={}, y={}, {} by {} flags=0x{} enabled={}",
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
        if self.cur_mode != 0 {
            error!("switching back from graphics mode is unimplemented");
            Err(-1)
        } else {
            debug!("already in VGA compatibility mode (text mode)");
            Ok(())
        }
    }
}

type State = RefCell<VirtioGpuDev>;
unsafe impl Send for VirtioGpuDev {}

extern "C" {
    fn _glue_vga_copy_out(dest: *mut u16, len: usize);
    fn _glue_vga_copy_in(src: *mut u16, len: usize);
}

impl gpudev::GpuDev for VirtioGpuDev {
    type State = State;

    fn get_available_modes(state: &Self::State, modes: &mut [VideoMode]) -> Result<usize> { 
        debug!("get_available_modes");

        let mut d = state.borrow_mut();

        if modes.len() < 2 {
            error!("Must provide at least two mode slots\n");
            return Err(-1);
        }

        if d.update_modes().is_err() {
            error!("Cannot update modes\n");
            return Err(-1);

        }
        // now translate modes back to that expected by the abstraction
        // we will interpret each scanout as a mode, plus add a text mode as well
        let limit = if modes.len() > 16 { 15 } else { modes.len() - 1 };
        let mut cur: usize = 0;

        modes[cur] = d.gen_mode(0);
        cur += 1;

        // graphics modes
        for i in 0..16 {
            if cur >= limit {
                break;
            }
            if d.disp_info_resp.pmodes[i].enabled != 0 {
                debug!("filling out entry {cur} with scanout info {i}");
                modes[cur] = d.gen_mode(i + 1);
                cur += 1;
            }
        }


        Ok(cur)
    }

    fn get_mode(state: &Self::State) -> Result<VideoMode> {
        debug!("get_mode");

        let d = state.borrow_mut();
        Ok(d.gen_mode(d.cur_mode))
    }
    
    

    // set a video mode based on the modes discovered
    // this will switch to the mode before returning
    fn set_mode(
        state: &Self::State, 
        mode: &VideoMode
    ) -> Result {
        {
        let mut d = state.borrow_mut();
        let mode_num = mode.mode_data as usize;

        debug!("set mode on {}", d.name());

        // 1. First, clean up the current mode and get us back to
        //    the basic text mode

        if d.cur_mode == 0 {
            // we are in VGA text mode - capture the text on screen
            unsafe { _glue_vga_copy_out(d.text_snapshot.as_mut_ptr(), 80 * 25 * 2); }
            debug!("copy out of text mode data complete");
        }

        // reset ourselves back to text mode before doing a switch
        if d.reset().is_err() {
            error!("Cannot reset device");
            return Err(-1);
        } 

        debug!("reset complete");

        if mode_num == 0 {
            // we are switching back to VGA text mode - restore
            // the text on the screen
            unsafe {_glue_vga_copy_in(d.text_snapshot.as_ptr() as _, 80 * 25 * 2); }
            debug!("copy in of text mode data complete");
            debug!("switch to text mode complete");
            return Ok(());
        }

        // if we got here, we are switching to a graphics mode

        // we are switching to this graphics mode
        let pm = &d.disp_info_resp.pmodes[mode_num - 1] as *const DisplayOne as *mut DisplayOne;

        // 2. we next create a resource for the screen
        //    use SCREEN_RID as the ID

        let mut create_2d_req = ResourceCreate2d::default();
        let mut create_2d_resp = CtrlHdr::default();

        create_2d_req.hdr.type_ = VirtioGpuCtrlType::ResourceCreate2D;
        create_2d_req.resource_id = SCREEN_RID;
        create_2d_req.format = VirtioGpuFormat::R8G8B8A8Unorm as u32;
        create_2d_req.width = unsafe { pm.read().r.width };
        create_2d_req.height = unsafe { pm.read().r.height };


        debug!("doing transaction to create 2D screen");

        unsafe {
            transact_rw(
                &mut *d.virtio_dev,
                0,
                &create_2d_req,
                &mut create_2d_resp,
            ).inspect_err(|_| error!("failed to create 2D screen (transaction failed"))?;
        }

        check_response(&create_2d_resp, VirtioGpuCtrlType::OkNoData, "failed to create 2D screen")?;
        debug!("transaction complete");

        // 3. we would create a framebuffer that we can write pixels into

        let num_pixels = unsafe { (pm.read().r.width * pm.read().r.height) as usize };

        let frame_buffer = (vec![Pixel::default(); num_pixels]).into_boxed_slice();
        d.frame_buffer = Some(frame_buffer);


        let fb_length = num_pixels * core::mem::size_of::<Pixel>();
        debug!("allocated screen framebuffer of length {fb_length}");

        // now create a description of it in a bounding box
        d.frame_box = Rect {
            x: 0,
            y: 0,
            width: unsafe { pm.read().r.width },
            height: unsafe { pm.read().r.height },
        };

        // make the clipping box the entire screen
        d.clipping_box = Rect {
            x: 0,
            y: 0,
            width: unsafe { pm.read().r.width },
            height: unsafe { pm.read().r.height },
        };

        // 4. we should probably fill the framebuffer with some initial data
        // A typical driver would fill it with zeros (black screen), but we
        // might want to put something more exciting there.

        // (the default pixel values are black, so we've already done this).

        // 5. Now we need to associate our framebuffer (step 4) with our resource (step 2)

        let mut backing_req = ResourceAttachBacking::default();
        let mut backing_entry = MemEntry::default();
        let mut backing_resp = CtrlHdr::default();

        backing_req.hdr.type_ = VirtioGpuCtrlType::ResourceAttachBacking;
        backing_req.resource_id = SCREEN_RID;
        backing_req.nr_entries = 1;

        backing_entry.addr = d.frame_buffer.as_ref().unwrap().as_ptr() as *const c_void as u64;
        backing_entry.length = fb_length as _;


        debug!("doing transaction to associate framebuffer with screen resource");
        unsafe {
            transact_rrw(
                &mut *d.virtio_dev,
                0,
                &backing_req,
                &backing_entry,
                &mut backing_resp,
            )
        }.inspect_err(|_| error!("failed to associate framebuffer with screen resource (transaction failed)"))?;

        check_response(&backing_resp,
                       VirtioGpuCtrlType::OkNoData,
                       "failed to associate framebuffer with screen resource")?;

        debug!("transaction complete");

        // 6. Now we need to associate our resource (step 2) with the scanout (step 1)
        //    use mode_num-1 as the scanout ID

        let mut setso_req = SetScanout::default();
        let mut setso_resp = CtrlHdr::default();

        setso_req.hdr.type_ = VirtioGpuCtrlType::SetScanout;
        setso_req.resource_id = SCREEN_RID;
        setso_req.r = unsafe { pm.read().r };
        setso_req.scanout_id = mode_num as u32 - 1;

        debug!("doing transaction to associate screen resource with the scanout");
        unsafe {
            transact_rw(
                &mut *d.virtio_dev,
                0,
                &setso_req,
                &mut setso_resp,
            )
        }.inspect_err(|_| error!("failed to associate screen resource with the scanout (transaction failed)"))?;

        check_response(&setso_resp,
                       VirtioGpuCtrlType::OkNoData,
                       "failed to associate screen resource with the scanout")?;

        debug!("transaction complete");

        // Now let's capture our mode number to indicate we are done with setup
        // and make subsequent calls aware of our state
        d.cur_mode = mode_num;

        } // lock guard is dropped

        Self::flush(state)?;

        Ok(())
    }

    // drawing commands
    
    // each of these is asynchronous - the implementation should start the operation
    // but not necessarily finish it.   In particular, nothing needs to be drawn
    // until flush is invoked

    // flush - wait until all preceding drawing commands are visible by the user
    fn flush(state: &Self::State) -> Result {
        debug!("flush");

        let mut d = state.borrow_mut();
        if d.cur_mode == 0 {
            debug!("ignoring flush for text mode");
            return Ok(());
        }

        // First, tell the GPU to DMA from our framebuffer to the resource

        let mut xfer_req = TransferToHost2D::default();
        let mut xfer_resp = CtrlHdr::default();

        xfer_req.hdr.type_ = VirtioGpuCtrlType::TransferToHost2D;
        xfer_req.r = d.disp_info_resp.pmodes[d.cur_mode - 1].r;
        xfer_req.offset = 0;
        xfer_req.resource_id = SCREEN_RID;

        debug!("beginning transaction to tell GPU to DMA from framebuffer\n");

        unsafe {
            transact_rw(
                &mut *d.virtio_dev,
                0,
                &xfer_req,
                &mut xfer_resp,
            ).inspect_err(|_| error!("failed to tell GPU to DMA from framebuffer (transaction failed)"))?;
        }

        check_response(&xfer_resp, VirtioGpuCtrlType::OkNoData, "failed to tell GPU to DMA from framebuffer")?;
        debug!("transaction complete");

        // Second, tell the GPU to copy from the resource to the screen
        let mut flush_req = ResourceFlush::default();
        let mut flush_resp = CtrlHdr::default();

        flush_req.hdr.type_ = VirtioGpuCtrlType::ResourceFlush;
        flush_req.r = d.disp_info_resp.pmodes[d.cur_mode - 1].r;
        flush_req.resource_id = SCREEN_RID;

        debug!("beginning transaction to tell GPU to copy from resource to screen");
        unsafe {
            transact_rw(
                &mut *d.virtio_dev,
                0,
                &flush_req,
                &mut flush_resp,
            ).inspect_err(|_| error!("failed to tell GPU to copy from resource to screen (transaction failed)"))?;
        }

        check_response(&flush_resp,
                       VirtioGpuCtrlType::OkNoData,
                       "failed to tell GPU to copy from resource to screen\n")?;
        debug!("transaction complete");

        // User should now see the changes
        Ok(())
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
    fn graphics_set_clipping_box(state: &Self::State, rect: Option<&Rect>) -> Result {
        let mut d = state.borrow_mut();

        debug!("graphics_set_clipping_box on {}: {:?})\n", d.name(), rect);

        d.clipping_box = rect.map(|rect| *rect).unwrap_or(d.frame_box);

        Ok(())
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
        let mut d = state.borrow_mut();
        debug!("graphics_fill_box_with_bitmap on {}", d.name());

        for i in 0..(rect.width) {
            for j in 0..(rect.height) {
                *d.get_pixel(rect.x + i, rect.y + j) = *get_bitmap_pixel(bitmap, i % bitmap.width, j % bitmap.height).unwrap();
            }
        }

        Ok(())
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

        debug_dump_descriptors(vq, 0, 8);

        // ask the virtio-pci subsystem we live in to actually do the
        // notification write
        bindings::virtio_pci_virtqueue_notify(dev as *mut _, qidx);

        // The device has our request now

        debug!("request initiated");

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

    debug!("transaction complete");

    Ok(())
}

unsafe fn transact_rw<R, W>(
    dev: &mut bindings::virtio_pci_dev,
    qidx: u16,
    req: &R,
    resp: &mut W
) -> Result {
    let mut desc_idx = [0_u16; 2];
    let reqlen = core::mem::size_of::<R>() as u32;
    let resplen = core::mem::size_of::<W>() as u32;

    unsafe {
        // allocate a two element descriptor chain, the descriptor
        // numbers will be placed in the desc_idx array.
        if bindings::virtio_pci_desc_chain_alloc(dev as *mut _, qidx, desc_idx.as_mut_ptr(), 2) != 0 {
            error!("Failed to allocate descriptor chain");
            return Err(-1);
        }

        debug!("allocated chain {} -> {}", desc_idx[0], desc_idx[1]);

        // Now get pointers to the specific descriptors in the virtq struct
        // (which is shared with the hardware)
        let desc = [dev.virtq[qidx as usize].vq.desc.offset(desc_idx[0] as isize),
                    dev.virtq[qidx as usize].vq.desc.offset(desc_idx[1] as isize)];

        // now build a linked list of 2 elements in this space

        // this is the "read" part - the request
        // first element of the linked list
        (*desc[0]).addr = req as *const _ as u64;
        (*desc[0]).len = reqlen;
        (*desc[0]).flags |= 0;
        (*desc[0]).next = desc_idx[1];  // next pointer is next descriptor
                                        //
        // this is the "write" part - the response
        // this is where we want the device to put the response
        (*desc[1]).addr = resp as *mut _ as u64;
        (*desc[1]).len = resplen;
        (*desc[1]).flags |= bindings::VIRTQ_DESC_F_WRITE as u16;
        (*desc[1]).next = 0;            // next pointer is null   
    }

    unsafe { transact_base(dev, qidx, desc_idx[0]) }
}

unsafe fn transact_rrw<R1, R2, W>(
    dev: &mut bindings::virtio_pci_dev,
    qidx: u16,
    req: &R1,
    more: &R2,
    resp: &mut W
) -> Result {
    let mut desc_idx = [0_u16; 3];
    let reqlen =  core::mem::size_of::<R1>() as u32;
    let morelen = core::mem::size_of::<R2>() as u32;
    let resplen = core::mem::size_of::<W>() as u32;

    unsafe {
        // allocate a two element descriptor chain, the descriptor
        // numbers will be placed in the desc_idx array.
        if bindings::virtio_pci_desc_chain_alloc(dev as *mut _, qidx, desc_idx.as_mut_ptr(), 3) != 0 {
            error!("Failed to allocate descriptor chain");
            return Err(-1);
        }

        debug!("allocated chain {} -> {} -> {}", desc_idx[0], desc_idx[1], desc_idx[2]);

        // Now get pointers to the specific descriptors in the virtq struct
        // (which is shared with the hardware)
        let desc = [dev.virtq[qidx as usize].vq.desc.offset(desc_idx[0] as isize),
                    dev.virtq[qidx as usize].vq.desc.offset(desc_idx[1] as isize),
                    dev.virtq[qidx as usize].vq.desc.offset(desc_idx[2] as isize)];

        // this is the "read" part - the request
        // first element of the linked list
        (*desc[0]).addr = req as *const _ as u64;
        (*desc[0]).len = reqlen;
        (*desc[0]).flags |= 0;
        (*desc[0]).next = desc_idx[1];  // next pointer is next descriptor

        // more readable data, but perhaps in a different, non-consecutive address
        (*desc[1]).addr = more as *const _ as u64;
        (*desc[1]).len = morelen;
        (*desc[1]).flags |= 0;
        (*desc[1]).next = desc_idx[2];  // next pointer is next descriptor

        // this is the "write" part - the response
        // this is where we want the device to put the response
        (*desc[2]).addr = resp as *mut _ as u64;
        (*desc[2]).len = resplen;
        (*desc[2]).flags |= bindings::VIRTQ_DESC_F_WRITE as u16;
        (*desc[2]).next = 0;            // next pointer is null   
    }

    unsafe { transact_base(dev, qidx, desc_idx[0]) }
}

fn debug_dump_descriptors(vq: &bindings::virtq, start: usize, count: usize) {
    for i in start..(start + count) {
        unsafe {
            let addr  = vq.desc.offset(i as _).read().addr;
            let len   = vq.desc.offset(i as _).read().len;
            let flags = vq.desc.offset(i as _).read().flags;
            let next  = vq.desc.offset(i as _).read().next;
            debug!("vq[{}] = {:#x} len={} flags={:#x} next={}", i, addr, len, flags, next);
        }
    }
}

fn check_response(hdr: &CtrlHdr, expected: VirtioGpuCtrlType, error_message: &str) -> Result {
    if hdr.type_ == expected {
        Ok(())
    } else {
        debug!("hdr = {:?}", hdr);
        error!("{}", error_message);
        Err(-1)
    }
}

#[no_mangle]
extern "C" fn virtio_gpu_init(virtio_dev: *mut bindings::virtio_pci_dev) -> core::ffi::c_int {
    info!("init");

    // Allocate a default state structure for this device
    let dev = Arc::new(RefCell::new(VirtioGpuDev::default()));

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
    let dev_ptr = &*dev.borrow_mut() as *const _ as *mut VirtioGpuDev;
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
