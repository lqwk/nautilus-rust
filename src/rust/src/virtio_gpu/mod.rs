#![allow(unused_variables)]

use core::ffi::c_void;
use core::ops::Not;

use crate::kernel::{
    bindings, gpudev,
    sync::Spinlock,
    gpudev::{BitBlitOp, Bitmap, Char, Coordinate, Font, Pixel, Rect, Region, VideoMode},
    print::make_logging_macros,
};
use crate::prelude::*;

make_logging_macros!("virtio_gpu", NAUT_CONFIG_DEBUG_VIRTIO_GPU);

// "scanout" means monitor
const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

// the resource ids we will use
// it is important to note that resource id 0 has special
// meaning - it means "disabled" or "none"
const SCREEN_RID: u32 = 42;

/*
 *  Structs and enums used in  device transactions.
 *
 *  These structs must be `#[repr(C)]`, as the layout must
 *  exactly match the device's expectation, and enums
 *  used in these structs must also have the proper `repr`
 *  (e.g. `#[repr(i32)]`).
 */

/// The different requests and responses that can appear in
/// device transactions.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(i32)]
enum CtrlType {
    #[default]
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

/// Support Pixel formats.
#[allow(dead_code)]
#[repr(u32)]
enum PixelFormatUnorm {
    B8G8R8A8 = 1,
    B8G8R8X8 = 2,
    A8R8G8B8 = 3,
    X8R8G8B8 = 4,
    R8G8B8A8 = 67,
    X8B8G8R8 = 68,
    A8B8G8R8 = 121,
    R8G8B8X8 = 134,
}

/// All requests and responses include this
/// header as their first (and sometimes only) part
#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
struct CtrlHdr {
    type_: CtrlType,
    flags: u32,
    fence_id: u64,
    ctx_id: u32,
    padding: u32,
}

/// A rectangular box. This is in practice equivalent to the `Rect`
/// struct (an alias to `bindings::nk_gpu_dev_box_t`) provided in the
/// generic GPU interface, but we define this here so that we will
/// always match the Virtio GPU devices expectations in transactions,
/// even if the generic GPU `Rect` struct's layout were to change.
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
    flags: u32,
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
    resource_id: u32, // we need to supply the id, it cannot be zero
    format: u32,      // pixel format (as above)
    width: u32,       // resource size in pixels
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
    padding: u32,
}

#[derive(Default)]
#[repr(C)]
struct ResourceFlush {
    hdr: CtrlHdr,
    r: GpuRect,
    resource_id: u32,
    padding: u32,
}

/*
 *  Virtio GPU state and associated functions.
 */

/// The types of video modes. This is really just a renaming of the
/// enum produced by bindgen and should probably be defined in `gpudev.rs`
/// or by tweaking bindgen's parameters.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VideoModeType {
    Text = bindings::nk_gpu_dev_video_mode_NK_GPU_DEV_MODE_TYPE_TEXT as _,
    Graphics = bindings::nk_gpu_dev_video_mode_NK_GPU_DEV_MODE_TYPE_GRAPHICS_2D as _,
}

/// The current mode of the device. Text mode: 0, Graphics mode: > 0.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurModeType {
    Text,
    Graphics(usize),
}

/// CurModeType enum conversion functions for use in referencing pmodes array.
impl CurModeType {
    /// Create a `CurModeType` from a `usize`. 
    fn from_usize(n: usize) -> CurModeType {
        match n {
            0 => CurModeType::Text,
            _ => CurModeType::Graphics(n),
        }
    }
    /// Convert a `CurModeType` to a `usize`.
    fn to_usize(&self) -> usize {
        match self {
            CurModeType::Text => 0,
            CurModeType::Graphics(n) => *n,
        }
    }
}

/// The information we associate with the Virtio GPU device
struct VirtioGpu {
    /// The handle to the GPU device registration.
    gpu_dev: Option<gpudev::Registration<Self>>,
    /// The device as recognized by the PCI subsystem.
    pci_dev: *mut bindings::virtio_pci_dev,
    have_disp_info: bool,
    /// data from the last request for modes made of the device
    disp_info_resp: RespDisplayInfo,
    /// (cur_mode == 0) => text mode; (cur_mode > 0) => graphics mode.
    cur_mode: CurModeType,
    /// The in-memory pixel data.
    frame_buffer: Option<Box<[Pixel]>>,
    /// A bounding box descibing the frame buffer.
    frame_box: Rect,
    /// A bounding box that restricts drawing.
    clipping_box: Rect,
    /// A snapshot of text-mode data, so that we can save and later restore
    /// when switching between VGA text-mode and graphics.
    text_snapshot: [u16; 80 * 25],
}

impl Default for VirtioGpu {
    fn default() -> VirtioGpu {
        VirtioGpu {
            gpu_dev: None,
            pci_dev: core::ptr::null_mut(),
            have_disp_info: false,
            disp_info_resp: RespDisplayInfo::default(),
            cur_mode: CurModeType::Text,
            frame_buffer: None,
            frame_box: Rect::default(),
            clipping_box: Rect::default(),
            text_snapshot: [0; 80 * 25],
        }
    }
}

impl VirtioGpu {
    /// Gets a mutable reference to pixel at (`x`, `y`).
    /// This function does not check if (`x`, `y`) is a valid coordinate in the frame box.
    ///
    /// # Panics
    ///
    /// Will panic if the device's frame buffer was initialized (via `set_mode`)
    /// or if (`x`, `y`) is not within the frame buffer.
    fn get_pixel(&mut self, x: u32, y: u32) -> &'_ mut Pixel {
        &mut self.frame_buffer.as_mut().unwrap()[(y * self.frame_box.width + x) as usize]
    }

    /// Creates a `VideoMode` based on the given mode number (0 => text; >0 => graphics).
    fn gen_mode(&self, modenum: CurModeType) -> VideoMode {
        if modenum == CurModeType::Text {
            VideoMode {
                type_: VideoModeType::Text as _,
                width: 80,
                height: 25,
                channel_offset: [0, 1, 0xFF, 0xFF],
                flags: 0,
                mouse_cursor_width: 0,
                mouse_cursor_height: 0,
                mode_data: modenum.to_usize() as *mut c_void,
            }
        } else {
            VideoMode {
                type_: VideoModeType::Graphics as _,
                width: self.disp_info_resp.pmodes[modenum.to_usize() - 1].r.width,
                height: self.disp_info_resp.pmodes[modenum.to_usize() - 1].r.height,
                channel_offset: [0, 1, 2, 3],
                flags: bindings::NK_GPU_DEV_HAS_MOUSE_CURSOR as _,
                mouse_cursor_width: 64,
                mouse_cursor_height: 64,
                mode_data: modenum.to_usize() as *mut c_void,
            }
        }
    }

    /// Updates display information by talking to the Virtio GPU device.
    fn update_modes(&mut self) -> Result {
        if self.have_disp_info {
            return Ok(());
        }

        let disp_info_req = CtrlHdr {
            type_: CtrlType::GetDisplayInfo,
            ..Default::default()
        };
        self.disp_info_resp = RespDisplayInfo::default();

        // SAFETY: PCI subsystem ensures `virtio_dev` is a valid pointer when it
        // gives it to us in `virtio_gpu_init`.
        unsafe {
            transact_rw(
                &mut *self.pci_dev,
                0,
                &disp_info_req,
                &mut self.disp_info_resp,
            )?;
        }

        check_response(
            &self.disp_info_resp.hdr,
            CtrlType::OkDisplayInfo,
            "Failed to get display info",
        )?;

        for (i, mode) in self.disp_info_resp.pmodes.iter().enumerate() {
            if mode.enabled != 0 {
                debug!(
                    "scanout (monitor) {} has info: x={}, y={}, {} by {} flags=0x{} enabled={}",
                    i, mode.r.x, mode.r.y, mode.r.width, mode.r.height, mode.flags, mode.enabled
                );
            }
        }

        self.have_disp_info = true;

        Ok(())
    }
    // This function resets the pipeline we have created
    // Switching back from graphics mode in unimplemented
    fn reset(&mut self) -> Result {
        if self.cur_mode != CurModeType::Text {
            error!("switching back from graphics mode is unimplemented");
            Err(-1)
        } else {
            debug!("already in VGA compatibility mode (text mode)");
            Ok(())
        }
    }

    fn name(&self) -> &'_ str {
        self.gpu_dev.as_ref().unwrap().name()
    }
}

// SAFETY: The only reason this is needed is because of the `pci_dev` pointer,
// but this is always valid and does not have thread affinity.
unsafe impl Send for VirtioGpu {}

/*
 *  Helper functions.
 */

/// Gets a mutable reference to the pixel at (`x`, `y`) in the bitmap,
/// if that location is within the bitmap.
fn get_bitmap_pixel(bitmap: &Bitmap, x: u32, y: u32) -> Option<&'_ Pixel> {
    if x >= bitmap.width || y >= bitmap.height {
        None
    } else {
        // SAFETY: Caller ensures bitmap.pixels is a valid pointer
        // to bitmap.width * bitmap.height pixels.
        unsafe {
            Some(
                &bitmap
                    .pixels
                    .as_slice((bitmap.width * bitmap.height) as usize)
                    [(x + y * (bitmap.width)) as usize],
            )
        }
    }
}

/// Computes `oldpixel` = `op`(`oldpixel`, `newpixel`).
fn apply_with_blit(oldpixel: &mut Pixel, newpixel: &Pixel, op: BitBlitOp) {
    match op {
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_COPY => {
            // SAFETY: Both pixels are unions over integer fields, and
            // no invariants can be broken by this assignment.
            unsafe { oldpixel.raw = newpixel.raw }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_NOT => {
            // SAFETY: See above.
            unsafe { oldpixel.raw = newpixel.raw.not() }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_AND => {
            // SAFETY: See above.
            unsafe { oldpixel.raw &= newpixel.raw }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_OR => {
            // SAFETY: See above.
            unsafe { oldpixel.raw |= newpixel.raw }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_NAND => {
            // SAFETY: See above.
            unsafe { oldpixel.raw = (oldpixel.raw & newpixel.raw).not() }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_NOR => {
            // SAFETY: See above.
            unsafe { oldpixel.raw = (oldpixel.raw | newpixel.raw).not() }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_XOR => {
            // SAFETY: See above.
            unsafe { oldpixel.raw ^= newpixel.raw }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_XNOR => {
            // SAFETY: See above.
            unsafe { oldpixel.raw = (oldpixel.raw ^ newpixel.raw).not() }
        },
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_PLUS => {
            for i in 0..4 {
                // SAFETY: See above.
                unsafe {
                    oldpixel.channel[i] = oldpixel.channel[i].saturating_add(newpixel.channel[i])
                };
            }
        }
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_MINUS => {
            for i in 0..4 {
                // SAFETY: See above.
                unsafe {
                    oldpixel.channel[i] = oldpixel.channel[i].saturating_sub(newpixel.channel[i])
                };
            }
        }
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_MULTIPLY => {
            for i in 0..4 {
                // SAFETY: See above.
                unsafe {
                    oldpixel.channel[i] = oldpixel.channel[i].saturating_mul(newpixel.channel[i])
                };
            }
        }
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_DIVIDE => {
            for i in 0..4 {
                // SAFETY: See above.
                let rhs = unsafe { newpixel.channel[i] };
                if rhs == 0 {
                    // SAFETY: See above.
                    unsafe { oldpixel.channel[i] = u8::MAX };
                    } else {
                    // SAFETY: See above.
                    unsafe { oldpixel.channel[i] = oldpixel.channel[i].saturating_div(rhs) };
                }
            }
        }
    }
}

/// Checks if `location` is within `rect`.
fn in_rect(rect: &Rect, location: &Coordinate) -> bool {
    location.x >= rect.x
        && location.x < (rect.x + rect.width)
        && location.y >= rect.y
        && location.y < (rect.y + rect.height)
}

/// Applies `op` only if `location` is within the `clipping_box`.
fn clip_apply_with_blit(
    clipping_box: &Rect,
    location: &Coordinate,
    oldpixel: &mut Pixel,
    newpixel: &Pixel,
    op: BitBlitOp,
) {
    if in_rect(clipping_box, location) {
        apply_with_blit(oldpixel, newpixel, op);
    } else {
        debug!("failed to clip_apply_with_blit, location is not in rect");
    }
}

/// Draws `newpixel` into `oldpixel`, with clipping considerations.
fn draw_pixel(clipping_box: &Rect, location: &Coordinate, oldpixel: &mut Pixel, newpixel: &Pixel) {
    clip_apply_with_blit(
        clipping_box,
        location,
        oldpixel,
        newpixel,
        BitBlitOp::NK_GPU_DEV_BIT_BLIT_OP_COPY,
    );
}

/*
 *  `GpuDev` trait implementation.
 */

impl gpudev::GpuDev for VirtioGpu {
    type State = Spinlock<VirtioGpu>;

    fn get_available_modes(state: &Self::State, modes: &mut [VideoMode]) -> Result<usize> {
        debug!("get_available_modes");

        let mut d = state.lock();

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
        let limit = if modes.len() > 16 {
            15
        } else {
            modes.len() - 1
        };
        let mut cur: usize = 0;

        modes[cur] = d.gen_mode(CurModeType::Text);
        cur += 1;

        // graphics modes
        for i in 0..16 {
            if cur >= limit {
                break;
            }
            if d.disp_info_resp.pmodes[i].enabled != 0 {
                debug!("filling out entry {cur} with scanout info {i}");
                modes[cur] = d.gen_mode(CurModeType::from_usize(i + 1));
                cur += 1;
            }
        }

        Ok(cur)
    }
    // grab the current mode - useful in case you need to reset it later
    fn get_mode(state: &Self::State) -> Result<VideoMode> {
        debug!("get_mode");

        let d = state.lock();
        Ok(d.gen_mode(d.cur_mode))
    }

    // set a video mode based on the modes discovered
    // this will switch to the mode before returning
    fn set_mode(state: &Self::State, mode: &VideoMode) -> Result {
        {
            let mut d = state.lock();
            let mode_num = mode.mode_data as usize;

            debug!("set mode on {}", d.name());

            // 1. First, clean up the current mode and get us back to
            //    the basic text mode

            if d.cur_mode == CurModeType::Text {
                // we are in VGA text mode - capture the text on screen

                // SAFETY: FFI call.
                unsafe {
                    _glue_vga_copy_out(d.text_snapshot.as_mut_ptr(), 80 * 25 * 2);
                }
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

                // SAFETY: FFI call.
                unsafe {
                    _glue_vga_copy_in(d.text_snapshot.as_ptr() as _, 80 * 25 * 2);
                }
                debug!("copy in of text mode data complete");
                debug!("switch to text mode complete");
                return Ok(());
            }

            // if we got here, we are switching to a graphics mode
            //
            // (we use a raw pointer because the borrow checker sucks at understanding inner borrows).
            let pm = &d.disp_info_resp.pmodes[mode_num - 1] as *const DisplayOne as *mut DisplayOne;

            // 2. we next create a resource for the screen
            //    use SCREEN_RID as the ID

            let create_2d_req = ResourceCreate2d {
                hdr: CtrlHdr {
                    type_: CtrlType::ResourceCreate2D,
                    ..Default::default()
                },
                resource_id: SCREEN_RID,
                format: PixelFormatUnorm::R8G8B8A8 as u32,
                // SAFETY: `pm` is a valid pointer, as it was just created above.
                width: unsafe { pm.read().r.width },
                // SAFETY: `pm` is a valid pointer, as it was just created above.
                height: unsafe { pm.read().r.height },
            };
            let mut create_2d_resp = CtrlHdr::default();

            debug!("doing transaction to create 2D screen");

            // SAFETY: PCI subsystem ensures `pci_dev` is a valid pointer when it
            // gives it to us in `virtio_gpu_init`.
            unsafe {
                transact_rw(&mut *d.pci_dev, 0, &create_2d_req, &mut create_2d_resp)
                    .inspect_err(|_| error!("failed to create 2D screen (transaction failed"))?;
            }

            check_response(
                &create_2d_resp,
                CtrlType::OkNoData,
                "failed to create 2D screen",
            )?;
            debug!("transaction complete");

            // 3. we would create a framebuffer that we can write pixels into

            // SAFETY: `pm` is a valid pointer, as it was just created above.
            let num_pixels = unsafe { (pm.read().r.width * pm.read().r.height) as usize };

            let frame_buffer = (vec![Pixel::default(); num_pixels]).into_boxed_slice();
            d.frame_buffer = Some(frame_buffer);

            let fb_length = num_pixels * core::mem::size_of::<Pixel>();
            debug!("allocated screen framebuffer of length {fb_length}");

            // now create a description of it in a bounding box
            d.frame_box = Rect {
                x: 0,
                y: 0,
                // SAFETY: `pm` is a valid pointer, as it was just created above.
                width: unsafe { pm.read().r.width },
                // SAFETY: `pm` is a valid pointer, as it was just created above.
                height: unsafe { pm.read().r.height },
            };

            // make the clipping box the entire screen
            d.clipping_box = Rect {
                x: 0,
                y: 0,
                // SAFETY: `pm` is a valid pointer, as it was just created above.
                width: unsafe { pm.read().r.width },
                // SAFETY: `pm` is a valid pointer, as it was just created above.
                height: unsafe { pm.read().r.height },
            };

            // 4. we should probably fill the framebuffer with some initial data
            // A typical driver would fill it with zeros (black screen), but we
            // might want to put something more exciting there.

            // (the default pixel values are black, so we've already done this).

            // 5. Now we need to associate our framebuffer (step 4) with our resource (step 2)

            let backing_req = ResourceAttachBacking {
                hdr: CtrlHdr { 
                    type_: CtrlType::ResourceAttachBacking, 
                    ..Default::default()
                },
                resource_id: SCREEN_RID,
                nr_entries: 1,
            };

            let backing_entry = MemEntry {
                addr: d.frame_buffer.as_ref().ok_or(-1)?.as_ptr() as *const c_void as u64,
                length: fb_length as _,
                ..Default::default()
            };
            let mut backing_resp = CtrlHdr::default();

            debug!("doing transaction to associate framebuffer with screen resource");

            // SAFETY: PCI subsystem ensures `pci_dev` is a valid pointer when it
            // gives it to us in `virtio_gpu_init`.
            unsafe {
                transact_rrw(
                    &mut *d.pci_dev,
                    0,
                    &backing_req,
                    &backing_entry,
                    &mut backing_resp,
                )
            }
            .inspect_err(|_| {
                error!("failed to associate framebuffer with screen resource (transaction failed)")
            })?;

            check_response(
                &backing_resp,
                CtrlType::OkNoData,
                "failed to associate framebuffer with screen resource",
            )?;

            debug!("transaction complete");

            // 6. Now we need to associate our resource (step 2) with the scanout (step 1)
            //    use mode_num-1 as the scanout ID

            let setso_req = SetScanout {
                hdr: CtrlHdr {
                    type_: CtrlType::SetScanout,
                    ..Default::default()
                },
                resource_id: SCREEN_RID,
                // SAFETY: `pm` is a valid pointer, as it was just created above.
                r: unsafe { pm.read().r },
                scanout_id: mode_num as u32 - 1,
            };
            let mut setso_resp = CtrlHdr::default();

            debug!("doing transaction to associate screen resource with the scanout");

            // SAFETY: PCI subsystem ensures `pci_dev` is a valid pointer when it
            // gives it to us in `virtio_gpu_init`.
            unsafe { transact_rw(&mut *d.pci_dev, 0, &setso_req, &mut setso_resp) }
                .inspect_err(|_| {
                    error!(
                        "failed to associate screen resource with the scanout (transaction failed)"
                    )
                })?;

            check_response(
                &setso_resp,
                CtrlType::OkNoData,
                "failed to associate screen resource with the scanout",
            )?;

            debug!("transaction complete");

            // Now let's capture our mode number to indicate we are done with setup
            // and make subsequent calls aware of our state
            d.cur_mode = CurModeType::from_usize(mode_num);
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

        let mut d = state.lock();
        if d.cur_mode == CurModeType::Text {
            debug!("ignoring flush for text mode");
            return Ok(());
        }

        // First, tell the GPU to DMA from our framebuffer to the resource

        let xfer_req = TransferToHost2D {
            hdr: CtrlHdr { type_: CtrlType::TransferToHost2D,
                 ..Default::default()},
            r: d.disp_info_resp.pmodes[d.cur_mode.to_usize() - 1].r,
            offset: 0,
            resource_id: SCREEN_RID,
            ..Default::default()
        };
        let mut xfer_resp = CtrlHdr::default();

        debug!("beginning transaction to tell GPU to DMA from framebuffer\n");

        // SAFETY: PCI subsystem ensures `pci_dev` is a valid pointer when it
        // gives it to us in `virtio_gpu_init`.
        unsafe {
            transact_rw(&mut *d.pci_dev, 0, &xfer_req, &mut xfer_resp).inspect_err(|_| {
                error!("failed to tell GPU to DMA from framebuffer (transaction failed)")
            })?;
        }

        check_response(
            &xfer_resp,
            CtrlType::OkNoData,
            "failed to tell GPU to DMA from framebuffer",
        )?;
        debug!("transaction complete");

        // Second, tell the GPU to copy from the resource to the screen
        let flush_req = ResourceFlush {
            hdr: CtrlHdr { type_: CtrlType::ResourceFlush, 
            ..Default::default()},
            r: d.disp_info_resp.pmodes[d.cur_mode.to_usize() - 1].r,
            resource_id: SCREEN_RID,
            ..Default::default()
        };
        let mut flush_resp = CtrlHdr::default();

        debug!("beginning transaction to tell GPU to copy from resource to screen");

        // SAFETY: PCI subsystem ensures `virtio_dev` is a valid pointer when it
        // gives it to us in `virtio_gpu_init`.
        unsafe {
            transact_rw(&mut *d.pci_dev, 0, &flush_req, &mut flush_resp).inspect_err(|_| {
                error!("failed to tell GPU to copy from resource to screen (transaction failed)")
            })?;
        }

        check_response(
            &flush_resp,
            CtrlType::OkNoData,
            "failed to tell GPU to copy from resource to screen\n",
        )?;
        debug!("transaction complete");

        // User should now see the changes
        Ok(())
    }

    // text mode drawing commands
    fn text_set_char(state: &Self::State, location: &Coordinate, val: &Char) -> Result {
        debug!("text_set_char on {}", state.lock().name());
        unimplemented!();
    }

    // cursor location in text mode
    fn text_set_cursor(state: &Self::State, location: &Coordinate, flags: u32) -> Result {
        debug!("text_set_cursor on {}", state.lock().name());
        unimplemented!();
    }

    // graphics mode drawing commands

    
    // confine drawing to this box or region
    fn graphics_set_clipping_box(state: &Self::State, rect: Option<&Rect>) -> Result {
        let mut d = state.lock();

        debug!("graphics_set_clipping_box on {}: {:?})\n", d.name(), rect);

        d.clipping_box = rect.copied().unwrap_or(d.frame_box);

        Ok(())
    }

    // confine drawing to this region overriding any previous regions or boxes
    // or should remove clipping limitations (reset to full screen size)
    fn graphics_set_clipping_region(state: &Self::State, region: &Region) -> Result {
        debug!(
            "graphics_set_clipping_region on {}",
            state.lock().name()
        );
        unimplemented!();
    }

    // Helper function:  oldpixel = op(oldpixel,newpixel) if in clipping box
    // else does nothing
    fn clip_apply_with_blit(
        state: &Self::State,
        location: &Coordinate,
        oldpixel: &mut Pixel,
        newpixel: &Pixel,
        op: BitBlitOp,
    ) -> Result {
        let d = state.lock();

        clip_apply_with_blit(&d.clipping_box, location, oldpixel, newpixel, op);

        Ok(())
    }

    // draw stuff
    fn graphics_draw_pixel(state: &Self::State, location: &Coordinate, pixel: &Pixel) -> Result {
        let mut d = state.lock();

        debug!(
            "graphics_draw_pixel {:?} on {} at ({}, {})",
            // SAFETY: `pixel` is a union over integer fields, and
            // no invariants can be broken by this read.
            unsafe { pixel.raw },
            d.name(),
            location.x,
            location.y
        );

        // location needs to be within the bounding box of the frame buffer
        // and pixel is only drawn if within the clipping box

        let clipping_box = d.clipping_box;
        let oldpixel = d.get_pixel(location.x, location.y);

        draw_pixel(&clipping_box, location, oldpixel, pixel);

        Ok(())
    }

    // draw line within bounding box of frame buffer limited to the portion
    // of the line that is within the clipping box
    fn graphics_draw_line(
        state: &Self::State,
        start: &Coordinate,
        end: &Coordinate,
        pixel: &Pixel,
    ) -> Result {
        let mut d = state.lock();

        debug!(
            "draw_line {:#x} on {} ({}, {}) to ({}, {}",
            // SAFETY: `pixel` is a union over integer fields, and
            // no invariants can be broken by this read.
            unsafe { pixel.raw },
            d.name(),
            start.x,
            start.y,
            end.x,
            end.y
        );

        // Bresenham's line algorithm, adapted from
        // https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm#All_cases

        let (mut x0, x1, mut y0, y1) = (start.x as i32, end.x as i32, start.y as i32, end.y as i32);

        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let sx = (x1 - x0).signum();
        let sy = (y1 - y0).signum();
        let mut error = dx + dy;

        loop {
            let location = Coordinate {
                x: x0 as u32,
                y: y0 as u32,
            };
            let clipping_box = d.clipping_box;
            let oldpixel = d.get_pixel(location.x, location.y);
            draw_pixel(&clipping_box, &location, oldpixel, pixel);

            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * error;
            if e2 >= dy {
                if x0 == x1 {
                    break;
                }
                error += dy;
                x0 += sx;
            }
            if e2 <= dx {
                if y0 == y1 {
                    break;
                }
                error += dx;
                y0 += sy;
            }
        }

        Ok(())
    }

    // draw poly within bounding box of frame buffer limited to the portion
    // of the poly that is within the clipping box
    fn graphics_draw_poly(state: &Self::State, coord_list: &[Coordinate], pixel: &Pixel) -> Result {
        debug!("graphics_draw_poly on {}", state.lock().name());

        for i in 0..coord_list.len() {
            Self::graphics_draw_line(
                state,
                &coord_list[i],
                &coord_list[(i + 1) % coord_list.len()],
                pixel,
            )?;
        }

        Ok(())
    }

    // draw box filled with pixel withing bounding box of frame buffer limited
    // to the portion of the box that is within the clipping box
    fn graphics_fill_box_with_pixel(
        state: &Self::State,
        rect: &Rect,
        pixel: &Pixel,
        op: BitBlitOp,
    ) -> Result {
        let mut d = state.lock();

        debug!(
            "graphics_fill_box_with_pixel {:#x} on {} with ({}, {}) ({}, {}) with op {:?}",
            // SAFETY: `pixel` is a union over integer fields, and
            // no invariants can be broken by this read.
            unsafe { pixel.raw },
            d.name(),
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.width,
            op
        );

        for i in 0..rect.width {
            for j in 0..rect.height {
                let location = Coordinate {
                    x: rect.x + i,
                    y: rect.y + j,
                };

                let clipping_box = d.clipping_box;
                let oldpixel = d.get_pixel(location.x, location.y);

                clip_apply_with_blit(&clipping_box, &location, oldpixel, pixel, op);
            }
        }

        Ok(())  
    }

    // copy from the bitmap to the frame buffer using the op to transform (via bitblit) the 
    // output pixels that are withing the bounding box of the frame buffer limited to the
    // portion that is within the clipping box
    fn graphics_fill_box_with_bitmap(
        state: &Self::State,
        rect: &Rect,
        bitmap: &Bitmap,
        op: BitBlitOp,
    ) -> Result {
        let mut d = state.lock();
        debug!("graphics_fill_box_with_bitmap on {}", d.name());

        for i in 0..(rect.width) {
            for j in 0..(rect.height) {
                let location = Coordinate {
                    x: rect.x + i,
                    y: rect.y + j,
                };
                let clipping_box = d.clipping_box;
                let oldpixel = d.get_pixel(location.x, location.y);
                let pixel = get_bitmap_pixel(bitmap, i % bitmap.width, j % bitmap.height).ok_or(-1)?;
                clip_apply_with_blit(&clipping_box, &location, oldpixel, pixel, op);
            }
        }

        Ok(())
    }

    // copy from one box in the frame buffer to another box in the frame buffer using the op to
    // transform (via bitblit) the output pixels that are withing the bounding box of the frame
    // buffer limited to the portion that is within the clipping box
    fn graphics_copy_box(
        state: &Self::State,
        src_box: &Rect,
        dest_box: &Rect,
        op: BitBlitOp,
    ) -> Result {
        let mut d = state.lock();

        debug!("graphics_copy_box on {}", d.name());

        for i in 0..dest_box.width {
            for j in 0..dest_box.height {
                let old_location = Coordinate {
                    x: dest_box.x + i,
                    y: dest_box.y + j,
                };
                let new_location = Coordinate {
                    x: src_box.x + (i % src_box.width),
                    y: src_box.y + (j % src_box.height),
                };
                let clipping_box = d.clipping_box;

                if !in_rect(&clipping_box, &old_location) {
                    break;
                }

                if !in_rect(&clipping_box, &new_location) {
                    break;
                }

                let newpixel = *d.get_pixel(new_location.x, new_location.y);
                let oldpixel = d.get_pixel(old_location.x, old_location.y);

                clip_apply_with_blit(&clipping_box, &old_location, oldpixel, &newpixel, op);
            }
        }

        Ok(())
    }

    // draw text to vc, if supported
    fn graphics_draw_text(
        state: &Self::State,
        location: &Coordinate,
        font: &Font,
        text: &str,
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

/*
 *  DMA transactions with the Virtio GPU device.
 */

extern "C" {
    fn _glue_mbarrier();
    fn _glue_virtio_pci_atomic_store_u16(destptr: *mut u16, value: u16);
    fn _glue_virtio_pci_atomic_load_u16(srcptr: *mut u16) -> u16;
    fn _glue_vga_copy_out(dest: *mut u16, len: usize);
    fn _glue_vga_copy_in(src: *mut u16, len: usize);
}

unsafe fn transact_base(dev: &mut bindings::virtio_pci_dev, qidx: u16, didx: u16) -> Result {
    let virtq = &mut (dev.virtq[qidx as usize]);
    let vq = &mut (virtq.vq);

    // SAFETY: The authors have done their best to follow the C implementation of this
    // function and defer discussions of invariants upheld to that driver.
    unsafe {
        // the following steps push didx onto the virtqueue
        // in a manner acceptable to the hardware
        (*vq.avail)
            .ring
            .as_mut_ptr()
            .add(((*vq.avail).idx % vq.qsz) as _)
            .write(didx);
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
        if bindings::virtio_pci_desc_chain_free(dev as *mut _, qidx, didx) != 0 {
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
    resp: &mut W,
) -> Result {
    let mut desc_idx = [0_u16; 2];
    let reqlen = core::mem::size_of::<R>() as u32;
    let resplen = core::mem::size_of::<W>() as u32;

    // SAFETY: The authors have done their best to follow the C implementation of this
    // function and defer discussions of invariants upheld to that driver.
    unsafe {
        // allocate a two element descriptor chain, the descriptor
        // numbers will be placed in the desc_idx array.
        if bindings::virtio_pci_desc_chain_alloc(dev as *mut _, qidx, desc_idx.as_mut_ptr(), 2) != 0
        {
            error!("Failed to allocate descriptor chain");
            return Err(-1);
        }

        debug!("allocated chain {} -> {}", desc_idx[0], desc_idx[1]);

        // Now get pointers to the specific descriptors in the virtq struct
        // (which is shared with the hardware)
        let desc = [
            dev.virtq[qidx as usize]
                .vq
                .desc
                .add(desc_idx[0] as _),
            dev.virtq[qidx as usize]
                .vq
                .desc
                .add(desc_idx[1] as _),
        ];

        // now build a linked list of 2 elements in this space

        // this is the "read" part - the request
        // first element of the linked list
        (*desc[0]).addr = req as *const _ as u64;
        (*desc[0]).len = reqlen;
        (*desc[0]).flags |= 0;
        (*desc[0]).next = desc_idx[1]; // next pointer is next descriptor
                                       //
                                       // this is the "write" part - the response
                                       // this is where we want the device to put the response
        (*desc[1]).addr = resp as *mut _ as u64;
        (*desc[1]).len = resplen;
        (*desc[1]).flags |= bindings::VIRTQ_DESC_F_WRITE as u16;
        (*desc[1]).next = 0; // next pointer is null

        transact_base(dev, qidx, desc_idx[0])
    }

}

unsafe fn transact_rrw<R1, R2, W>(
    dev: &mut bindings::virtio_pci_dev,
    qidx: u16,
    req: &R1,
    more: &R2,
    resp: &mut W,
) -> Result {
    let mut desc_idx = [0_u16; 3];
    let reqlen = core::mem::size_of::<R1>() as u32;
    let morelen = core::mem::size_of::<R2>() as u32;
    let resplen = core::mem::size_of::<W>() as u32;

    // SAFETY: The authors have done their best to follow the C implementation of this
    // function and defer discussions of invariants upheld to that driver.
    unsafe {
        // allocate a two element descriptor chain, the descriptor
        // numbers will be placed in the desc_idx array.
        if bindings::virtio_pci_desc_chain_alloc(dev as *mut _, qidx, desc_idx.as_mut_ptr(), 3) != 0
        {
            error!("Failed to allocate descriptor chain");
            return Err(-1);
        }

        debug!(
            "allocated chain {} -> {} -> {}",
            desc_idx[0], desc_idx[1], desc_idx[2]
        );

        // Now get pointers to the specific descriptors in the virtq struct
        // (which is shared with the hardware)
        let desc = [
            dev.virtq[qidx as usize]
                .vq
                .desc
                .add(desc_idx[0] as _),
            dev.virtq[qidx as usize]
                .vq
                .desc
                .add(desc_idx[1] as _),
            dev.virtq[qidx as usize]
                .vq
                .desc
                .add(desc_idx[2] as _),
        ];

        // this is the "read" part - the request
        // first element of the linked list
        (*desc[0]).addr = req as *const _ as u64;
        (*desc[0]).len = reqlen;
        (*desc[0]).flags |= 0;
        (*desc[0]).next = desc_idx[1]; // next pointer is next descriptor

        // more readable data, but perhaps in a different, non-consecutive address
        (*desc[1]).addr = more as *const _ as u64;
        (*desc[1]).len = morelen;
        (*desc[1]).flags |= 0;
        (*desc[1]).next = desc_idx[2]; // next pointer is next descriptor

        // this is the "write" part - the response
        // this is where we want the device to put the response
        (*desc[2]).addr = resp as *mut _ as u64;
        (*desc[2]).len = resplen;
        (*desc[2]).flags |= bindings::VIRTQ_DESC_F_WRITE as u16;
        (*desc[2]).next = 0; // next pointer is null

        transact_base(dev, qidx, desc_idx[0])
    }
}

fn debug_dump_descriptors(vq: &bindings::virtq, start: usize, count: usize) {
    for i in start..(start + count) {
        // SAFETY: `vq.desc` is a valid pointer, ensured by PCI subsystem's
        // creation of `virtq`s.
        unsafe {
            let addr = vq.desc.add(i).read().addr;
            let len = vq.desc.add(i).read().len;
            let flags = vq.desc.add(i).read().flags;
            let next = vq.desc.add(i).read().next;
            debug!(
                "vq[{}] = {:#x} len={} flags={:#x} next={}",
                i, addr, len, flags, next
            );
        }
    }
}

/// Checks that `hdr` contains the `expected` response.
fn check_response(hdr: &CtrlHdr, expected: CtrlType, error_message: &str) -> Result {
    if hdr.type_ == expected {
        Ok(())
    } else {
        debug!("hdr = {:?}", hdr);
        error!("{}", error_message);
        Err(-1)
    }
}

/*
 *  Device initialization through the PCI subsystem.
 */

#[no_mangle]
extern "C" fn virtio_gpu_init(virtio_dev: *mut bindings::virtio_pci_dev) -> core::ffi::c_int {
    info!("init");

    // Allocate a default state structure for this device
    let dev = Arc::new(Spinlock::new(VirtioGpu::default()));

    // Acknowledge to the device that we see it
    //
    // SAFETY: `virtio_dev` is a valid pointer, ensured by caller (PCI subsystem).
    if unsafe { bindings::virtio_pci_ack_device(virtio_dev) } != 0 {
        error!("Could not acknowledge device");
        return -1;
    }

    // Ask the device for what features it supports
    //
    // SAFETY: `virtio_dev` is a valid pointer, ensured by caller (PCI subsystem).
    if unsafe { bindings::virtio_pci_read_features(virtio_dev) } != 0 {
        error!("Unable to read device features");
        return -1;
    }

    // Tell the device what features we will support.
    //
    // We will not support either VIRGL (3D) or EDID (better display info) for now.
    //
    // SAFETY: `virtio_dev` is a valid pointer, ensured by caller (PCI subsystem).
    if unsafe { bindings::virtio_pci_write_features(virtio_dev, 0) } != 0 {
        error!("Unable to write device features");
        return -1;
    }

    // Initilize the device's virtqs. The virtio-gpu device
    // has two of them.  The first is for most requests/responses,
    // while the second is for (mouse) cursor updates and movement
    //
    // SAFETY: `virtio_dev` is a valid pointer, ensured by caller (PCI subsystem).
    if unsafe { bindings::virtio_pci_virtqueue_init(virtio_dev) } != 0 {
        error!("failed to initialize virtqueues");
        return -1;
    }

    // Associate our state with the general virtio-pci device structure,
    // and vice-versa:
    let dev_ptr = &*dev.lock() as *const _ as *mut VirtioGpu;

    // SAFETY: `virtio_dev` is a valid pointer, ensured by caller (PCI subsystem).
    // `dev_ptr` is also valid, as we just created it from a reference above.
    unsafe {
        (*virtio_dev).state = dev_ptr as *mut _;
        (*virtio_dev).teardown = None;
        (*dev_ptr).pci_dev = virtio_dev;
    }

    // Register the GPU device. We will only support the first Virtio GPU device
    // (virtio-gpu0).
    let res = gpudev::Registration::<VirtioGpu>::try_new("virtio-gpu0", Arc::clone(&dev));
    match res {
        Ok(registration) => {
            // SAFETY: `dev_ptr` is valid, as we just created it from a reference above.
            unsafe { (*dev_ptr).gpu_dev = Some(registration); }
        },
        Err(e) => {
            return e;
        }
    }

    // Could enable interrupts for the device here, but it's not necessary for this
    // simple driver.

    0
}
