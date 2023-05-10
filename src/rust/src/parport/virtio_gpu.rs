use nautilus::{debug_print, error_print, info_print, spin_lock_irq_save, spin_unlock_irq_restore};
use nautilus::{gpudev::*, irq::*, pci::*, virtio_gpu::*};

// Wrappers for debug and other output so that
// they can be enabled/disabled at compile time using kernel
// build configuration (Kconfig)
#[cfg(not(feature = "debug_virtio_gpu"))]
macro_rules! debug_print {
    ($fmt:expr $(, $args:expr)*) => {};
}

macro_rules! info_print {
    ($fmt:expr $(, $args:expr)*) => {
        info_print!("virtio_gpu: {}", format_args!($fmt $(, $args)*));
    };
}

macro_rules! debug_print {
    ($fmt:expr $(, $args:expr)*) => {
        debug_print!("virtio_gpu: {}", format_args!($fmt $(, $args)*));
    };
}

macro_rules! error_print {
    ($fmt:expr $(, $args:expr)*) => {
        error_print!("virtio_gpu: {}", format_args!($fmt $(, $args)*));
    };
}

// Wrappers for locking the software state of a device
macro_rules! state_lock {
    ($state:expr) => {
        let _state_lock_flags = spin_lock_irq_save(&($state).lock);
    };
}

macro_rules! state_unlock {
    ($state:expr) => {
        spin_unlock_irq_restore(&($state).lock, _state_lock_flags);
    };
}

// Macros for manipulating feature bits on virtio pci devices
macro_rules! fbit_isset {
    ($features:expr, $bit:expr) => {
        ($features) & (0x01 << ($bit)) != 0
    };
}

macro_rules! fbit_setif {
    ($features_out:expr, $features_in:expr, $bit:expr) => {
        if fbit_isset!($features_in, $bit) {
            $features_out |= 0x01 << ($bit);
        }
    };
}

macro_rules! debug_fbit {
    ($features:expr, $bit:expr) => {
        if fbit_isset!($features, $bit) {
            debug_print!("feature bit set: {}\n", stringify!($bit));
        }
    };
}

// This next chunk of code imports the abstractions and data types
// defined in the virtio documentation for this device
type Le8 = u8;
type Le16 = u16;
type Le32 = u32;
type Le64 = u64;

// A virtio GPU may do 3D mode (VIRGL)
// and it may support extended display info (EDID)
// We will do neither
const VIRTIO_GPU_F_VIRGL: u32 = 0x1;
const VIRTIO_GPU_F_EDID: u32 = 0x2;

// We can ask the device for statistics
// You do not need to
#[repr(C)]
struct VirtioGpuConfig {
    events_read: Le32,
    events_clear: Le32,
    num_scanouts: Le32,
    reserved: Le32,
}

use crate::virtio_gpu::{Le32, Le64};

// This is very important - it enumerates
// the different requests that we can make of the device
// as well as its valid responses.
#[repr(u32)]
enum VirtioGpuCtrlType {
    /* 2d commands */
    VirtioGpuCmdGetDisplayInfo = 0x0100,
    VirtioGpuCmdResourceCreate2d,
    VirtioGpuCmdResourceUnref,

    VirtioGpuCmdSetScanout,
    VirtioGpuCmdResourceFlush,
    VirtioGpuCmdTransferToHost2d,
    VirtioGpuCmdResourceAttachBacking,
    VirtioGpuCmdResourceDetachBacking,
    VirtioGpuCmdGetCapsetInfo,
    VirtioGpuCmdGetCapset,
    VirtioGpuCmdGetEdid,

    /* cursor commands */
    VirtioGpuCmdUpdateCursor = 0x0300,
    VirtioGpuCmdMoveCursor,

    /* success responses */
    VirtioGpuRespOkNoData = 0x1100,
    VirtioGpuRespOkDisplayInfo,
    VirtioGpuRespOkCapsetInfo,
    VirtioGpuRespOkCapset,
    VirtioGpuRespOkEdid,

    /* error responses */
    VirtioGpuRespErrUnspec = 0x1200,
    VirtioGpuRespErrOutOfMemory,
    VirtioGpuRespErrInvalidScanoutId,
    VirtioGpuRespErrInvalidResourceId,
    VirtioGpuRespErrInvalidContextId,
    VirtioGpuRespErrInvalidParameter,
}

const VIRTIO_GPU_FLAG_FENCE: u32 = 1 << 0;

// All requests and responses include this
// header as their first (and sometimes only) part
#[repr(C)]
struct VirtioGpuCtrlHdr {
    typ: Le32,     // from VirtioGpuCtrlType
    flags: Le32,   // generally zero
    fence_id: Le64, // memory barrier - you can ignore
    ctx_id: Le32,  // zero
    padding: Le32,
}

// The following are for the
// VirtioGpuCmdGetDisplayInfo request
// which tells you about attached monitors and their
// capabilities

// "scanout" means monitor
const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

// monitors (and other things) are represented by virtio_gpu_rect
#[repr(C)]
struct VirtioGpuRect {
    x: Le32,
    y: Le32,
    width: Le32,
    height: Le32,
}

// the request for display information is simply
// a bare VirtioGpuCtrlHdr

// the response for display information is this
#[repr(C)]
struct VirtioGpuRespDisplayInfo {
    hdr: VirtioGpuCtrlHdr, // contains the return code in typ
    pmodes: [VirtioGpuDisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}

#[repr(C)]
struct VirtioGpuDisplayOne {
    r: VirtioGpuRect, // width+height and where it is placed in the space
    enabled: Le32,    // is it attached?
    flags: Le32,
}

use crate::virtio_gpu::{Le32, Le64};

#[repr(C)]
struct VirtioGpuGetEdid {
    hdr: VirtioGpuCtrlHdr,
    scanout: Le32,
    padding: Le32,
}

// the response for extended display information (EDID) is
// this.   You will not need this.
#[repr(C)]
struct VirtioGpuRespEdid {
    hdr: VirtioGpuCtrlHdr,
    size: Le32,
    padding: Le32,
    edid: [u8; 1024],
}

// The following are for the VIRTIO_GPU_CMD_RESOURCE_CREATE_2D
// request, which creates a graphics canvas resource within
// the GPU.   This canvas is then rendered onto
// a scanout/monitor
//

// The possible pixel formats for a resource
// B8G8R8X8 means "4 bytes per pixel, 1 byte of blue
// followed by 1 byte of green followed by 1 byte
// of red followed by 1 byte that is ignored"
#[repr(u32)]
enum VirtioGpuFormats {
    VirtioGpuFormatB8G8R8A8Unorm = 1,
    VirtioGpuFormatB8G8R8X8Unorm = 2,
    VirtioGpuFormatA8R8G8B8Unorm = 3,
    VirtioGpuFormatX8R8G8B8Unorm = 4,
    VirtioGpuFormatR8G8B8A8Unorm = 67,
    VirtioGpuFormatX8B8G8R8Unorm = 68,
    VirtioGpuFormatA8B8G8R8Unorm = 121,
    VirtioGpuFormatR8G8B8X8Unorm = 134,
}

// the resource (canvas) creation request
#[repr(C)]
struct VirtioGpuResourceCreate2d {
    hdr: VirtioGpuCtrlHdr,
    resource_id: Le32, // we need to supply the id, it cannot be zero
    format: Le32,      // pixel format (as above)
    width: Le32,       // resource size in pixels
    height: Le32,
}

// the response for create_2d is simply
// a bare VirtioGpuCtrlHdr

// The following is for a the VIRTIO_GPU_CMD_RESOURCE_UNREF
// request, which frees a graphics canvas resource within
// the GPU.

// the request
#[repr(C)]
struct VirtioGpuResourceUnref {
    hdr: VirtioGpuCtrlHdr,
    resource_id: Le32, // which resource we are freeing
    padding: Le32,
}


use crate::virtio_gpu::{Le32, Le64};

// A description of a region of memory
// the attach_backing request is followed by nr_entries of these
#[repr(C)]
struct VirtioGpuMemEntry {
    addr: Le64,   // the physical address of our region / framebuffer
    length: Le32, // length of the region in bytes
    padding: Le32,
}

// struct virtio_gpu_resource_attach_backing {
#[repr(C)]
struct VirtioGpuResourceAttachBacking {
    hdr: VirtioGpuCtrlHdr,
    resource_id: Le32, // which resource
    nr_entries: Le32,  // how many regions of memory
}

// the response for attach_backing is simply
// a bare VirtioGpuCtrlHdr

// The following is for a the VIRTIO_GPU_CMD_RESOURCE_DETACH_BACKING
// request, which disassociates the region(s) of memory
// we previously attached from a graphics canvas resource on the GPU.

// request
#[repr(C)]
struct VirtioGpuResourceDetachBacking {
    hdr: VirtioGpuCtrlHdr,
    resource_id: Le32, // the resource we are detaching all regions from
    padding: Le32,
}

// the response for detach_backing is simply
// a bare VirtioGpuCtrlHdr

// The following is for a the VIRTIO_GPU_CMD_SET_SCANOUT
// request, which ties a graphics canvas resource to
// a particular monitor (scanout).  The resource will
// be rendered into the scanout:
//
// framebuffer -> resource -> scanout -> eyeball
//

// request
// associate this resource with that scanout for
// this rectangle of its screen pixels
// having multiple resources "cover" the scanout (screen)
// is a way of accelerating things like windows with movie playback
#[repr(C)]
struct VirtioGpuSetScanout {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect, // for us, this will be the whole scanout
    scanout_id: Le32, // the monitor, current mode_num minus one
    //    (modes are 1-indexed, while scanout ids are 0-indexed)
    resource_id: Le32, // the resource
}

// struct virtio_gpu_transfer_to_host_2d {
#[repr(C)]
struct VirtioGpuTransferToHost2D {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect, // which part of the resource is being with our pixels
    offset: Le64,     // where to start fetching the data from us
    resource_id: Le32,
    padding: Le32,
}

// struct virtio_gpu_resource_flush {
#[repr(C)]
struct VirtioGpuResourceFlush {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    resource_id: Le32,
    padding: Le32,
}

// struct virtio_gpu_cursor_pos {
#[repr(C)]
struct VirtioGpuCursorPos {
    scanout_id: Le32, // monitor
    x: Le32,          // position
    y: Le32,
    padding: Le32,
}

// struct virtio_gpu_update_cursor {
#[repr(C)]
struct VirtioGpuUpdateCursor {
    hdr: VirtioGpuCtrlHdr,
    pos: VirtioGpuCursorPos,
    resource_id: Le32,
    hot_x: Le32,
    hot_y: Le32,
    padding: Le32,
}

fn teardown(dev: &mut virtio_pci_dev) {
    DEBUG!("teardown");

    // We would actually do frees, etc, here

    virtio_pci_virtqueue_deinit(dev);
}

// Our interrupt handler - the device will interrupt
// us whenever the state of a virtq changes.  This is how
// it notifies us of changes it has made.  We notify it
// when *we* make changes via the notification register
// it maps into the physical address space
fn interrupt_handler(exp: &mut excp_entry_t, vec: excp_vec_t, priv_data: &mut c_void) -> i32 {
    DEBUG!("interrupt invoked");

    // EXTRA CREDIT:  MAKE THE DEVICE INTERRUPT DRIVEN!
    // Your basic device driver will be synchronous, with one
    // outstanding transaction at time. Remove these limitations

    // see the parport code for why we must do this
    IRQ_HANDLER_END();

    0
}

// Given features the virtio-gpu device supports, this function will
// determine which ones the driver will also support.
fn select_features(features: u64) -> u64 {
    DEBUG!("device features: 0x{:0x}", features);
    DEBUG_FBIT(features, VIRTIO_GPU_F_VIRGL);
    DEBUG_FBIT(features, VIRTIO_GPU_F_EDID);

    // choose accepted features
    let mut accepted: u64 = 0;

    // we will not support either VIRGL (3D) or
    // EDID (better display info) for now
    // if we did, we would enable the following
    //FBIT_SETIF(accepted,features,VIRTIO_GPU_F_VIRGL);
    //FBIT_SETIF(accepted,features,VIRTIO_GPU_F_EDID);

    DEBUG!("features accepted: 0x{:0x}", accepted);
    accepted
}

// Debugging support - print out count descriptors within a virtq
// starting at a given position
fn debug_dump_descriptors(vq: &virtq, start: usize, count: usize) {
    for i in start..start + count {
        DEBUG!("vq[{}] = {:p} len={} flags=0x{:x} next={}", i, vq.desc[i].addr, vq.desc[i].len, vq.desc[i].flags, vq.desc[i].next);
    }
}

fn transact_base(dev: &mut VirtioPciDev, qidx: u16, didx: u16) -> Result<(), String> {
    let virtq = &mut dev.virtq[qidx as usize];
    let vq = &mut virtq.vq;
    let waitidx = vq.avail.idx;
    let usedidx;

    vq.avail.ring[vq.avail.idx as usize % vq.qsz as usize] = didx;
    mem::barrier();
    vq.avail.idx += 1;
    mem::barrier();

    virtio_pci_atomic_store(&dev.common.queue_select, qidx);
    virtio_pci_atomic_store(&dev.common.queue_enable, 1);

    virtio_pci_virtqueue_notify(dev, qidx)?;

    while {
        usedidx = virtio_pci_atomic_load(&virtq.vq.used.idx);
        usedidx != waitidx
    } {}

    if virtio_pci_desc_chain_free(dev, qidx, didx)? {
        return Err(String::from("Failed to free descriptor chain"));
    }

    Ok(())
}
