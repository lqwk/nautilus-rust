use crate::prelude::*;
use crate::kernel::{
    bindings, 
    print::make_logging_macros,
    gpudev::{GpuDev, VideoMode, Coordinate, Char, Rect, Region, Pixel, BitBlitOp, Bitmap, Font, Registration},
    sync::IRQLock,
};

make_logging_macros!("virtio");

use core::{
    ffi::{c_int, c_void, c_ushort,},
};

extern "C" {
    fn _glue_virtio_pci_atomic_load(destptr: &u16);
    fn _glue_virtio_pci_atomic_stpre(destptr: &u16, val: u16);
}

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;


pub enum VirtioGpuCtrlType {
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

struct GpuRect
{
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

struct DisplayOne {
    r: GpuRect,
    enabled: u32,
    flags: u32
}

// #[derive(Debug)]
struct RespDisplayInfo {
    hdr: CtrlHdr,
    pmodes: [DisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}


#[derive(Debug)]
pub struct VirtioGpuDev {
    gpu_dev: Option<Registration<Self>>,
    // virtio_dev: *mut bindings::virtio_pci_dev,
    // spinlock: UnsafeCell<bindings::spinlock_t>,
    have_disp_info: c_int,
    disp_info_resp: *mut RespDisplayInfo,
    cur_mode: c_int,
    frame_buffer: *mut c_void,
    frame_box: *mut Rect,
    clipping_box: *mut Rect,
    cursor_buffer: *mut c_void,
    cursor_box: *mut Rect,
    text_snapshot: [c_ushort; 80*25],
}

#[repr(i32)] 
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoModeType {
    Text = bindings::nk_gpu_dev_video_mode_NK_GPU_DEV_MODE_TYPE_TEXT as _,
    Graphics = bindings::nk_gpu_dev_video_mode_NK_GPU_DEV_MODE_TYPE_GRAPHICS_2D as _,
}

// gpu device-specific functions
impl VirtioGpuDev {

    unsafe extern "C" fn update_modes(&self) -> c_int {

        if self.have_disp_info == 0 {
            return 0;
        }

        let disp_info_req =
            CtrlHdr {
                type_: VirtioGpuCtrlType::GetDisplayInfo,
                ..Default::default()
            };
        
        if unsafe {
            transact_rw(
                0,
                0,
                &disp_info_req as *const _ as *mut c_void,
                core::mem::size_of::<CtrlHdr>() as u32,
                self.disp_info_resp as *mut c_void,
                core::mem::size_of::<RespDisplayInfo>() as u32,
            ) } != 0 {
                error!("Failed to get display info\n");
                return -1;
            }

            for (i, mode) in (*self.disp_info_resp).pmodes.iter().enumerate() {
                if mode.enabled != 0 {
                    debug!("scanout (monitor) {} has info: x={}, y={}, {} by {} flags=0x{} enabled={}\n",
                        i,
                        mode.r.x,
                        mode.r.y,
                        mode.r.width,
                        mode.r.height,
                        mode.flags,
                        mode.enabled);
                }
            }
        
        self.have_disp_info = 1;

        0
    }

    unsafe extern "C" fn fill_out_mode(
        &self,
        mode: *mut bindings::nk_gpu_dev_video_mode_t,
        modenum: u32
    )  {
        // let state = unsafe { &*(raw_state as *mut Self) };

        if modenum == 0  {
            let m: bindings::nk_gpu_dev_video_mode_t = 
            bindings::nk_gpu_dev_video_mode_t {
                type_: VideoModeType::Text as u32,
                width: 80 as u32,
                height: 25 as u32,
                channel_offset: [0, 1, u8::MAX, u8::MAX],
                flags: 0,
                mouse_cursor_width: 0,
                mouse_cursor_height: 0,
                mode_data: modenum as u64 as *mut c_void, // not sure if this is right
            };
            unsafe { *mode = m };
        }
        else {
            let m: bindings::nk_gpu_dev_video_mode_t = 
            bindings::nk_gpu_dev_video_mode_t {
                type_: VideoModeType::Graphics as u32,
                width: (*self.disp_info_resp).pmodes[modenum as usize - 1].r.width as u32,
                height: (*self.disp_info_resp).pmodes[modenum as usize - 1].r.height as u32,
                channel_offset: [0, 1, 2, 3], // RGBA
                flags: bindings::NK_GPU_DEV_HAS_MOUSE_CURSOR as u64,
                mouse_cursor_width: 64 as u32,
                mouse_cursor_height: 64 as u32,
                mode_data: modenum as u64 as *mut c_void, // not sure if this is right
            };
            unsafe { *mode = m };
        }
    }
}

type State = IRQLock<VirtioGpuDev>;
unsafe impl Send for VirtioGpuDev {}

impl GpuDev for VirtioGpuDev {

    type State = State;

    fn get_available_modes(
        state: &Self::State,
        modes: &mut [VideoMode],
    ) -> Result { // issue with return type here
        
        if modes.len() < 2 {
            error!("Must provide at least two mode slots\n");
            return Result::from_error_code(-1 as i32);
        }

        if unsafe { state.lock().update_modes() } == 0 { 
            error!("Cannot update modes\n");
            return Result::from_error_code(-1 as i32);
        }

        // now translate modes back to that expected by the abstraction
        // we will interpret each scanout as a mode, plus add a text mode as well
        let limit = if modes.len() > 16 { 15 } else { modes.len() - 1 };
        let cur: usize = 0;

        unsafe { state.lock().fill_out_mode(&mut modes[cur], 0); }
        cur += 1;

        // graphics modes
        for i in 0..16 {
            if cur < limit {
                break;
            }
            // if 
            unsafe { state.lock().fill_out_mode( &mut modes[cur], i+1); }
            cur += 1;
        }

        // return Result::Ok(0 as usize as i32);
        Result::from_error_code(0 as i32);

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

type PLACEHOLDER = i32;

unsafe extern "C" fn transact_base(
    dev: PLACEHOLDER, // THIS IS A VIRTIO_PCI_DEV TYPE WHICH WE NEED TO FIGURE OUT,
    qidx: u16,
    didx: u16,
) -> c_int {
    unimplemented!();
}

unsafe extern "C" fn transact_rw(
    dev: PLACEHOLDER, // THIS IS A VIRTIO_PCI_DEV TYPE WHICH WE NEED TO FIGURE OUT,
    qidx: u16,
    req: *mut c_void,
    reqlen: u32,
    resp: *mut c_void,
    resplen: u32,
) -> c_int {
    unimplemented!();
}

unsafe extern "C" fn transact_rrw(
    dev: PLACEHOLDER, //THIS IS A VIRTIO_PCI_DEV TYPE WHICH WE NEED TO FIGURE OUT,
    qidx: u16,
    req: *mut c_void,
    reqlen: u32,
    more: *mut c_void,
    morelen: u32,
    resp: *mut c_void,
    resplen: u32,
) -> c_int {
    unimplemented!();
}