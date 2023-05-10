/*
 * This file is part of the Nautilus AeroKernel developed
 * by the Hobbes and V3VEE Projects with funding from the
 * United States National Science Foundation and the Department of Energy.
 *
 * The V3VEE Project is a joint project between Northwestern University
 * and the University of New Mexico.  The Hobbes Project is a collaboration
 * led by Sandia National Laboratories that includes several national
 * laboratories and universities. You can find out more at:
 * http://www.v3vee.org  and
 * http://xstack.sandia.gov/hobbes
 *
 * Copyright (c) 2019  Peter Dinda, Alex van der Heijden, Conor Hetland
 * Copyright (c) 2019, The Intereaving Project <http://www.interweaving.org>
 *                     The Hobbes Project <http://xstack.sandia.gov/hobbes>
 * All rights reserved.
 *
 * Authors: Peter Dinda <pdinda@northwestern.edu>
 *          Alex van der Heijden
 *          Conor Hetland
 *
 * This is free software.  You are permitted to use,
 * redistribute, and modify it as specified in the file "LICENSE.txt".
 */

/*
  This is stub code for the CS 343 Driver Lab at Northwestern.

  This driver provides access to the modern virtio GPU interface,
  which numerous virtual machine monitors, including QEMU and KVM
  use to provide an emulated GPU for graphics, or to expose a real
  underlying hardware GPU.

  Virtio is a general mechanism for interfacing with VMM-provided
  devices.  Virtio-PCI is that mechanism instantiated for the PCI bus.
  Virtio-GPU is a driver for GPUs that talk via the PCI instantiation
  of Virtio.

  General specification of Virtio, Virtio-PCI, and Virtio-GPU:

  https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html

  Note that the documentation for virtio on osdev is for
  the "legacy" version.   The virtio drivers for block and network
  devices (virtio_net.c, virtio_blk.c) in NK are also for the
  "legacy" version.   It is important to note that Virtio-GPU is
  a "modern" device, and so while the concepts are similar, the
  implementation is a bit different.
*/



////////////////////////////////////////////////////////
// The following are for the VIRTIO_GPU_CMD_GET_EDID
// request, which can access extended display information
//

// the request for extended display information (EDID) is
// this.   You will not need this.

// the response for resource_unref is simply
// a bare struct virtio_gpu_ctrl_hdr


////////////////////////////////////////////////////////
// The following is for a the VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING
// request, which associates region(s) of memory with
// a graphics canvas resource on the GPU.
//
// For simple 2D graphics we will have just one region of memory,
// which we call the framebuffer.  We write pixels into the
// framebuffer, and then tell the GPU to transfer them to
// the relevant graphics canvas resource.  The GPU will
// do this using DMA

// Request



// the response for set_scanout is simply
// a bare struct virtio_gpu_ctrl_hdr


////////////////////////////////////////////////////////
// The following is for a the VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D
// request, which tells the GPU to copy the data (via DMA) within the
// framebuffer (or other backing regions) to the graphics canvas
// resource.
//
// framebuffer -> resource -> scanout -> eyeball
//

// request copies from the backing regions to the
// resource for the rectangle of pixels


//
// If we use the exact same structure for the request, but
// have the type be VIRTIO_GPU_MOVE_CURSOR, then we don't
// change the cursor image, just move it.   This is faster
// than a full update
//

// the response for an update_cursor is simply a bare struct
// virtio_gpu_ctrl_hdr (and optionally can be disabled)



////////////////////////////////////////////////////////
// Core functions of the driver follow
//
//

// this function will be invoked by the virtio framework
// within NK when it wants us to stop controlling the
// device



//
// Helper function to do a virtq transaction on the device
// you are indicating that for virtq qidx, you want to push
// descriptor didx into the available ring, and then let
// the device know changed the virtq.
//
//
// available ring is being pushed to
//    it will afterwards contain a pointer (didx) to
//      the first descriptor in a chain (linked list)
//
// We will then notify the device of this change
//
// Finally, we will wait for the device to push didx into
// the used ring, indicating it has finished the request
//
// In an interrupt-driven model, we would not do any waiting
// here.  Instead, it would be the interrupt handler that would
// be fired when the device moved didx to the used ring, and
// the handler would then need to inform the original caller somehow,
// probably through a callback function

    

// helper function to do a simple transaction using
// a descriptor chain of length 2.
//
//   descriptor 0:   read only  (contains request)
//   descriptor 1:   write only (where we want the response to go)
//
static int transact_rw(struct virtio_pci_dev *dev,
		       uint16_t qidx,
		       void    *req,
		       uint32_t reqlen,
		       void    *resp,
		       uint32_t resplen)
{
    uint16_t desc_idx[2];

    // allocate a two element descriptor chain, the descriptor
    // numbers will be placed in the desc_idx array.
    if (virtio_pci_desc_chain_alloc(dev, qidx, desc_idx, 2)) {
	ERROR("Failed to allocate descriptor chain\n");
	return -1;
    }

    //DEBUG("allocated chain %hu -> %hu\n",desc_idx[0],desc_idx[1]);

    
    
    // Now get pointers to the specific descriptors in the virtq struct
    // (which is shared with the hardware)
    struct virtq_desc *desc[2] = {&dev->virtq[qidx].vq.desc[desc_idx[0]],
				  &dev->virtq[qidx].vq.desc[desc_idx[1]]};

    // now build a linked list of 2 elements in this space

    // this is the "read" part - the request
    // first element of the linked list
    desc[0]->addr = (le64) req;
    desc[0]->len = reqlen;
    desc[0]->flags |= 0;
    desc[0]->next = desc_idx[1];  // next pointer is next descriptor

    // this is the "write" part - the response
    // this is where we want the device to put the response
    desc[1]->addr = (le64) resp;
    desc[1]->len = resplen;
    desc[1]->flags |= VIRTQ_DESC_F_WRITE;  
    desc[1]->next = 0;            // next pointer is null   
 
    return transact_base(dev,qidx,desc_idx[0]);
}

// helper function to do a simple transaction using
// a descriptor chain of length 3.
//
//   descriptor 0:   read only  (contains request)
//   descriptor 1:   read only  (contains more of the request (for variable length stuff))
//   descriptor 2:   write only (where we want the response to go)
//
static int transact_rrw(struct virtio_pci_dev *dev,
			uint16_t qidx,
			void    *req,
			uint32_t reqlen,
			void    *more,
			uint32_t morelen,
			void    *resp,
			uint32_t resplen)
{
    uint16_t desc_idx[3];

    // allocate a three element descriptor chain, the descriptor
    // numbers will be placed in the desc_idx array.
    if (virtio_pci_desc_chain_alloc(dev, qidx, desc_idx, 3)) {
	ERROR("Failed to allocate descriptor chain\n");
	return -1;
    }

    //    DEBUG("allocated chain %hu -> %hu -> %hu\n",desc_idx[0],desc_idx[1],desc_idx[2]);

    // Now get pointers to the specific descriptors in the virtq struct
    // (which is shared with the hardware)
    struct virtq_desc *desc[3] = {&dev->virtq[qidx].vq.desc[desc_idx[0]],
				  &dev->virtq[qidx].vq.desc[desc_idx[1]],
				  &dev->virtq[qidx].vq.desc[desc_idx[2]] };

    // this is the "read" part - the request
    // first element of the linked list
    desc[0]->addr = (le64) req;
    desc[0]->len = reqlen;
    desc[0]->flags |= 0;
    desc[0]->next = desc_idx[1];  // next pointer is next descriptor

    // more readable data, but perhaps in a different, non-consecutive address
    desc[1]->addr = (le64) more;
    desc[1]->len = morelen;
    desc[1]->flags |= 0;
    desc[1]->next = desc_idx[2]; // next pointer is next descriptor

    // this is the "write" part - the response
    // this is where we want the device to put the response
    desc[2]->addr = (le64) resp;
    desc[2]->len = resplen;
    desc[2]->flags |= VIRTQ_DESC_F_WRITE;
    desc[2]->next = 0;           // next pointer is null

    return transact_base(dev,qidx,desc_idx[0]);
}


///////////////////////////////////////////////////////////////////
// We can support multiple virtio-gpu devices - this variable
// is usd to create an enumeration
//
static uint64_t num_devs = 0;

///////////////////////////////////////////////////////////////////
// The software state of a device
//
struct virtio_gpu_dev {
    struct nk_gpu_dev           *gpu_dev;     // we are a gpu device
    
    struct virtio_pci_dev       *virtio_dev;  // we are also a virtio pci device

    spinlock_t                   lock;        // we have a lock

    // data from the last request for modes made of the device
    int                                 have_disp_info;
    struct virtio_gpu_resp_display_info disp_info_resp;

    // if cur_mode==0, it means we are in normal text mode
    // if cur_mode>0, then we are in some graphics mode
    int                                 cur_mode; // 0 => text, otherwise cur_mode-1 => offset into disp_info_resp

    void                        *frame_buffer;   // will point to your in-memory pixel data, array of nk_gpu_dev_pixel_t
    nk_gpu_dev_box_t             frame_box;      // a bounding box that describes your framebuffer
    nk_gpu_dev_box_t             clipping_box;   // a bounding box that restricts drawing

    void                        *cursor_buffer;  // for EC - mouse cursor frame buffer
    nk_gpu_dev_box_t             cursor_box;     // for EC - bounding box describing mouse cursor frame buffer

    uint16_t                     text_snapshot[80*25];  // so we can save/restore vga text-mode data
};


// helper to zero requests - always a good idea!
#define ZERO(a) memset(a,0,sizeof(*a))

// the resource ids we will use
// it is important to note that resource id 0 has special
// meaning - it means "disabled" or "none"
#define SCREEN_RID 42     // for the whole screen (scanout)
#define CURSOR_RID 23     // for the mouse cursor (if implemented)

// helper macro to make sure that response we get are quickly and easily checked
#define CHECK_RESP(h,ok,errstr) if (h.type!=ok) { ERROR(errstr " rc=%x\n",h.type); return -1; }

#define DEV_NAME(s) ((s)->gpu_dev->dev.name)

#define UNIMPL() ERROR("unimplemented\n"); return -1;


// gpu device-specific functions

static int update_modes(struct virtio_gpu_dev *d)
{

    if (d->have_disp_info) {
	return 0;
    }

    
    // Our request/response pair (response stored in device struct)
    struct virtio_gpu_ctrl_hdr disp_info_req;

    // Be paranoid about these things - you want them to start with all zeros
    ZERO(&disp_info_req);
    ZERO(&d->disp_info_resp);

    // we are making the get display info request
    disp_info_req.type = VIRTIO_GPU_CMD_GET_DISPLAY_INFO;

    // now issue the request via virtqueue
    if (transact_rw(d->virtio_dev,
                    0,
                    &disp_info_req,
                    sizeof(disp_info_req),
                    &d->disp_info_resp,
                    sizeof(d->disp_info_resp)))
    {
        ERROR("Failed to get display info\n");
        return -1;
    }

    // If we get here, we have a response, but we don't know if the response is OK
    // ALWAYS CHECK
    CHECK_RESP(d->disp_info_resp.hdr, VIRTIO_GPU_RESP_OK_DISPLAY_INFO, "Failed to get display info");

    // now just print out the monitors and their resolutions
    for (int i = 0; i < 16; i++)  {
        if (d->disp_info_resp.pmodes[i].enabled) {
            DEBUG("scanout (monitor) %u has info: x=%u, y=%u, %u by %u flags=0x%x enabled=%d\n", i,
		  d->disp_info_resp.pmodes[i].r.x,
		  d->disp_info_resp.pmodes[i].r.y,
		  d->disp_info_resp.pmodes[i].r.width,
		  d->disp_info_resp.pmodes[i].r.height,
		  d->disp_info_resp.pmodes[i].flags,
		  d->disp_info_resp.pmodes[i].enabled);
        }
    }
    
    d->have_disp_info = true;
    
    return 0;
}


static void fill_out_mode(struct virtio_gpu_dev *d, nk_gpu_dev_video_mode_t *mode, int modenum)
{
    if (modenum == 0) { 
	// text mode
	nk_gpu_dev_video_mode_t m = {
	    .type = NK_GPU_DEV_MODE_TYPE_TEXT,
	    .width = 80,
	    .height = 25,
	    .channel_offset = { 0, 1, -1, -1 },
	    .flags = 0,
	    .mouse_cursor_width = 0,
	    .mouse_cursor_height = 0,
	    .mode_data = (void*)(uint64_t)modenum,
	};
	*mode = m;
    } else {
	nk_gpu_dev_video_mode_t m = {
	    .type = NK_GPU_DEV_MODE_TYPE_GRAPHICS_2D,
	    .width = d->disp_info_resp.pmodes[modenum-1].r.width,
	    .height = d->disp_info_resp.pmodes[modenum-1].r.height,
	    .flags = NK_GPU_DEV_HAS_MOUSE_CURSOR,
	    .channel_offset = { 0, 1, 2, 3 },  // RGBA
	    .mouse_cursor_width = 64,
	    .mouse_cursor_height = 64,
	    .mode_data = (void*)(uint64_t)modenum,
	};
	*mode = m;
    }
}
    
	
// discover the modes supported by the device
//     modes = array of modes on entry, filled out on return
//     num = size of array on entry, number of modes found on return
// 
static int get_available_modes(void *state,
			       nk_gpu_dev_video_mode_t modes[],
			       uint32_t *num)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("get_available_modes on %s\n", DEV_NAME(d));

    if (*num<2) {
	ERROR("Must provide at least two mode slots\n");
	return -1;
    }

    if (update_modes(d)) {
	ERROR("Cannot update modes\n");
	return -1;
    }

    // now translate modes back to that expected by the abstraction
    // we will interpret each scanout as a mode, plus add a text mode as well
    uint32_t limit = *num > 16 ? 15 : *num-1;
    uint32_t cur=0;

    fill_out_mode(d,&modes[cur++],0);

    // graphics modes
    for (int i = 0; i < 16 && cur < limit; i++) {
        if (d->disp_info_resp.pmodes[i].enabled)  {
	    DEBUG("filling out entry %d with scanout info %d\n",cur,i);
	    fill_out_mode(d,&modes[cur++],i+1);
	}
    }

    *num = cur;

    return 0;
}


// grab the current mode - useful in case you need to reset it later
static int get_mode(void *state, nk_gpu_dev_video_mode_t *mode)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    
    DEBUG("get_mode on %s\n", DEV_NAME(d));

    fill_out_mode(d,mode,d->cur_mode);

    return 0;
}

//
// This function resets the pipeline we have created
// it is completely written.   Take a look at it
// to see more examples of how to interact with the device
// 
//
static int reset(struct virtio_gpu_dev *d)
{
    if (d->cur_mode) {
	
	// detach framebuffer
	struct virtio_gpu_resource_detach_backing backing_detach_req;
	struct virtio_gpu_ctrl_hdr                backing_detach_resp;
	
	ZERO(&backing_detach_req);
	ZERO(&backing_detach_resp);
	
	backing_detach_req.hdr.type = VIRTIO_GPU_CMD_RESOURCE_DETACH_BACKING;
	backing_detach_req.resource_id = SCREEN_RID;

	if (transact_rw(d->virtio_dev,
			0,
			&backing_detach_req,
			sizeof(backing_detach_req),
			&backing_detach_resp,
			sizeof(backing_detach_resp))) {
	    ERROR("failed to detach screen framebuffer (transaction failed)\n");
	    return -1;
	}

	CHECK_RESP(backing_detach_resp, VIRTIO_GPU_RESP_OK_NODATA, "failed to detach screen framebuffer\n");

	DEBUG("detached screen framebuffer\n");

	// unref resource
	struct virtio_gpu_resource_unref unref_req;
	struct virtio_gpu_ctrl_hdr       unref_resp;

	ZERO(&unref_req);
	ZERO(&unref_resp);
	
	unref_req.hdr.type = VIRTIO_GPU_CMD_RESOURCE_UNREF;
	unref_req.resource_id = SCREEN_RID;
	
	if (transact_rw(d->virtio_dev,
			0,
			&unref_req,
			sizeof(unref_req),
			&unref_resp,
			sizeof(unref_resp))) {
	    ERROR("failed to unref screen resource (transaction failed)\n");
	    return -1;
	}
	
	CHECK_RESP(unref_resp, VIRTIO_GPU_RESP_OK_NODATA, "failed to unref screen resource\n");
	
	DEBUG("unreferenced screen resource\n");
	
	free(d->frame_buffer);
	d->frame_buffer = NULL;

	DEBUG("freed screen framebuffer\n");

	// detach cursor buffer
	// TODO: uncomment this code if you are doing mouse pointer extra credit
	/*
	ZERO(&backing_detach_req);
	ZERO(&backing_detach_resp);
	
	backing_detach_req.hdr.type = VIRTIO_GPU_CMD_RESOURCE_DETACH_BACKING;
	backing_detach_req.resource_id = CURSOR_RID;

	if (transact_rw(d->virtio_dev,
			0,
			&backing_detach_req,
			sizeof(backing_detach_req),
			&backing_detach_resp,
			sizeof(backing_detach_resp))) {
	    ERROR("failed to detach cursor framebuffer (transaction failed)\n");
	    return -1;
	}

	CHECK_RESP(backing_detach_resp, VIRTIO_GPU_RESP_OK_NODATA, "failed to detach cursor framebuffer\n");

	DEBUG("detached cursor framebuffer\n");

	ZERO(&unref_req);
	ZERO(&unref_resp);
	
	unref_req.hdr.type = VIRTIO_GPU_CMD_RESOURCE_UNREF;
	unref_req.resource_id = CURSOR_RID;
	
	if (transact_rw(d->virtio_dev,
			0,
			&unref_req,
			sizeof(unref_req),
			&unref_resp,
			sizeof(unref_resp))) {
	    ERROR("failed to unref cursor resource (transaction failed)\n");
	    return -1;
	}
	
	CHECK_RESP(unref_resp, VIRTIO_GPU_RESP_OK_NODATA, "failed to unref cursor resource\n");
	
	DEBUG("unreferenced cursor resource\n");
	
	free(d->cursor_buffer);
	d->cursor_buffer=0;

	DEBUG("freed cursor framebuffer\n");
	*/
	
	// attempt to reset to VGA text mode
	DEBUG("reseting device back to VGA compatibility mode (we hope - this will fail on older QEMUs)\n");

	// reset scanouts to disabled
	virtio_pci_atomic_store(&d->virtio_dev->common->device_status, 0);
	
	d->cur_mode = 0;
	
    } else {
	DEBUG("already in VGA compatibility mode (text mode)\n");
    }
    
    return 0;
}

static int flush(void *state);

// set a video mode based on the modes discovered
// this will switch to the mode before returning
static int set_mode(void *state, nk_gpu_dev_video_mode_t *mode)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    int mode_num = (int)(int64_t)(mode->mode_data);

    DEBUG("set_mode on %s\n", DEV_NAME(d));

    // 1. First, clean up the current mode and get us back to
    //    the basic text mode
    
    if (d->cur_mode==0) {
	// we are in VGA text mode - capture the text on screen
	vga_copy_out(d->text_snapshot,80*25*2);
	DEBUG("copy out of text mode data complete\n");
    }

    // reset ourselves back to text mode before doing a switch
    if (reset(d)) {
	ERROR("Cannot reset device\n");
	return -1;
    }

    DEBUG("reset complete\n");
    
    if (mode_num==0) {
	// we are switching back to VGA text mode - restore
	// the text on the screen
	vga_copy_in(d->text_snapshot,80*25*2);
	DEBUG("copy in of text mode data complete\n");
	DEBUG("switch to text mode complete\n");
	return 0;
    }

    // if we got here, we are switching to a graphics mode

    // we are switching to this graphics mode
    struct virtio_gpu_display_one *pm = &d->disp_info_resp.pmodes[mode_num-1];

    // 2. we next create a resource for the screen
    //    use SCREEN_RID as the ID

    struct virtio_gpu_resource_create_2d create_2d_req;
    struct virtio_gpu_ctrl_hdr           create_2d_resp;

    ZERO(&create_2d_req);
    ZERO(&create_2d_resp);

    //
    // WRITE ME!
    //

    create_2d_req.hdr.type = VIRTIO_GPU_CMD_RESOURCE_CREATE_2D;
    create_2d_req.resource_id = SCREEN_RID;
    // TODO: how to choose format?
    // VIRTIO_GPU_FORMAT_R8G8B8A8_UNORM seems best since is it is rgba
    // and works with NK_GPU_DEV_PIXEL_SET_RGBA
    create_2d_req.format = VIRTIO_GPU_FORMAT_R8G8B8A8_UNORM;
    create_2d_req.width = pm->r.width;
    create_2d_req.height = pm->r.height;

    DEBUG("doing transaction to create 2D screen\n");
    if (transact_rw(d->virtio_dev, 0,
                    &create_2d_req, sizeof(create_2d_req),
                    &create_2d_resp, sizeof(create_2d_resp))) {
        ERROR("failed to create 2D screen (transaction failed)\n");
        return -1;
    };
    CHECK_RESP(create_2d_resp, VIRTIO_GPU_RESP_OK_NODATA, "failed to create 2D screen\n");
    DEBUG("transaction complete\n");
    
    // 3. we would create a framebuffer that we can write pixels into

    uint64_t fb_length = pm->r.width * pm->r.height * sizeof(nk_gpu_dev_pixel_t);

    d->frame_buffer = malloc(fb_length);
    
    if (!d->frame_buffer) {
	ERROR("failed to allocate framebuffer of length %lu\n",fb_length);
	return -1;
    } else {
	DEBUG("allocated framebuffer of length %lu\n",fb_length);
    }
    
    DEBUG("allocated screen framebuffer of length %lu\n", fb_length);
    
    // now create a description of it in a bounding box
    d->frame_box.x=0;
    d->frame_box.y=0;
    d->frame_box.width=pm->r.width;
    d->frame_box.height=pm->r.height;

    // make the clipping box the entire screen
    d->clipping_box.x=0;
    d->clipping_box.y=0;
    d->clipping_box.width=pm->r.width;
    d->clipping_box.height=pm->r.height;

    // 4. we should probably fill the framebuffer with some initial data
    // A typical driver would fill it with zeros (black screen), but we
    // might want to put something more exciting there.

    //
    // WRITE ME
    //
    
    DEBUG("filling framebuffer with initial screen\n");
    
    memset(d->frame_buffer, 0, fb_length);

    // 5. Now we need to associate our framebuffer (step 4) with our resource (step 2)

    struct virtio_gpu_resource_attach_backing backing_req;
    struct virtio_gpu_mem_entry               backing_entry;
    struct virtio_gpu_ctrl_hdr                backing_resp;
    
    ZERO(&backing_req);
    ZERO(&backing_entry);
    ZERO(&backing_resp);

    //
    // WRITE ME
    //

    backing_req.hdr.type = VIRTIO_GPU_CMD_RESOURCE_ATTACH_BACKING;
    backing_req.resource_id = SCREEN_RID;
    backing_req.nr_entries = 1;

    backing_entry.addr = (le64) d->frame_buffer;
    backing_entry.length = fb_length;

    DEBUG("doing transaction to associate framebuffer with screen resource\n");
    if (transact_rrw(d->virtio_dev, 0,
                 &backing_req,   sizeof(backing_req),
                 &backing_entry, sizeof(backing_entry),
                 &backing_resp,  sizeof(backing_resp))) {
        ERROR("failed to associate framebuffer with screen resource (transaction failed)\n");
        return -1;
    };
    CHECK_RESP(backing_resp, VIRTIO_GPU_RESP_OK_NODATA,
               "failed to associate framebuffer with screen resource\n");
    DEBUG("transaction complete\n");
    
    // 6. Now we need to associate our resource (step 2) with the scanout (step 1)
    //    use mode_num-1 as the scanout ID

    struct virtio_gpu_set_scanout setso_req;
    struct virtio_gpu_ctrl_hdr    setso_resp;

    ZERO(&setso_req);
    ZERO(&setso_resp);

    //
    // WRITE ME
    //

    setso_req.hdr.type = VIRTIO_GPU_CMD_SET_SCANOUT;
    setso_req.resource_id = SCREEN_RID;
    setso_req.r = pm->r;
    setso_req.scanout_id = mode_num - 1;

    DEBUG("doing transaction to associate screen resource with the scanout\n");
    if (transact_rw(d->virtio_dev, 0,
                &setso_req, sizeof(setso_req),
                &setso_resp, sizeof(setso_resp))) {
        ERROR("failed to associate screen resource with the scanout (transaction failed)\n");
        return -1;
    };
    CHECK_RESP(setso_resp, VIRTIO_GPU_RESP_OK_NODATA,
               "failed to associate screen resource with the scanout\n");
    DEBUG("transaction complete\n");

    // Now let's capture our mode number to indicate we are done with setup
    // and make subsequent calls aware of our state
    //
    d->cur_mode = mode_num; 

    // Flush the pipeline  (note that you need to write flush!)
    flush(d);

    // we should now have whatever is in framebuffer on the screen

    //
    // EC: EXTRA CREDIT STARTS
    // 
    
    // EC: now we will set up the mouse cursor

    // EC: Create a resource for the mouse cursor bitmap
    //     use ID CURSOR_RID
    ZERO(&create_2d_req);
    ZERO(&create_2d_resp);

    //
    // EC: WRITE ME
    //

    //
    // EC: allocate a framebuffer for the mouse cursor
    //     These are always 64x64
    fb_length = 64*64*4;

    d->cursor_buffer = malloc(fb_length);

    if (!d->cursor_buffer) {
        ERROR("failed to allocate cursor framebuffer of length %lu (transaction failed)\n", fb_length);
	reset(d);
	return -1;
    }

    // EC: Now describe the mouse cursor framebuffer

    d->cursor_box.x=0;
    d->cursor_box.y=0;
    d->cursor_box.width=64;
    d->cursor_box.height=64;
    
    DEBUG("allocated cursor framebuffer of length %lu\n", fb_length);

    // EC: Now we would fill cursor_buffer with the initial cursor bitmap

    // EC: Next, we would attach cursor_buffer to resource we created
    
    ZERO(&backing_req);
    ZERO(&backing_entry);
    ZERO(&backing_resp);

    //
    // EC: WRITE ME
    //

    // EC: Next, we would move the mouse cursor to the middle of the screen
    
    DEBUG("set_mode complete\n");
    
    return 0;
}

// drawing commands

// each of these is asynchronous - the implementation should start the operation
// but not necessarily finish it.   In particular, nothing needs to be drawn
// until flush is invoked

// flush - wait until all preceding drawing commands are visible by the user
static int flush(void *state)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    
    DEBUG("flush on %s\n", DEV_NAME(d));

    if (d->cur_mode==0) {
	DEBUG("ignoring flush for text mode)\n");
	return 0;
    }

    struct virtio_gpu_display_one *pm = &d->disp_info_resp.pmodes[d->cur_mode-1];


    // First, tell the GPU to DMA from our framebuffer to the resource
    struct virtio_gpu_transfer_to_host_2d xfer_req;
    struct virtio_gpu_ctrl_hdr            xfer_resp;

    ZERO(&xfer_req);
    ZERO(&xfer_resp);

    //
    // WRITE ME
    //
    // (simple version: transfer whole framebuffer)
    // (complex version: transfer only the parts that have changed since that last flush)

    xfer_req.hdr.type = VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D;
    xfer_req.r = pm->r;
    xfer_req.offset = 0;
    xfer_req.resource_id = SCREEN_RID;

    DEBUG("beginning transaction to tell GPU to DMA from framebuffer\n");
    if (transact_rw(d->virtio_dev, 0,
                    &xfer_req, sizeof(xfer_req),
                    &xfer_resp, sizeof(xfer_resp))) {
        ERROR("failed to tell GPU to DMA from framebuffer (transaction failed)\n");
        return -1;
    }
    CHECK_RESP(xfer_resp, VIRTIO_GPU_RESP_OK_NODATA,
               "failed to tell GPU to DMA from framebuffer\n");
    DEBUG("transaction complete\n");

    // Second, tell the GPU to copy from the resource to the screen
    
    struct virtio_gpu_resource_flush flush_req;
    struct virtio_gpu_ctrl_hdr       flush_resp;

    ZERO(&flush_req);
    ZERO(&flush_resp);

    //
    // WRITE ME
    //
    flush_req.hdr.type = VIRTIO_GPU_CMD_RESOURCE_FLUSH;
    flush_req.r = pm->r;
    flush_req.resource_id = SCREEN_RID;

    DEBUG("beginning transaction to tell GPU to copy from resource to screen\n");
    if (transact_rw(d->virtio_dev, 0,
                    &flush_req, sizeof(flush_req),
                    &flush_resp, sizeof(flush_resp))) {
        ERROR("failed to tell GPU to copy from resource to screen (transaction failed)\n");
        return -1;
    }
    CHECK_RESP(flush_resp, VIRTIO_GPU_RESP_OK_NODATA,
               "failed to tell GPU to copy from resource to screen\n");
    DEBUG("transaction complete\n");


    // User should now see the changes

    return 0;
}

// text mode drawing commands
static int text_set_char(void *state, nk_gpu_dev_coordinate_t *location, nk_gpu_dev_char_t *val)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("text_set_char on %s\n", DEV_NAME(d));
    
    UNIMPL();
}

// cursor location in text mode
static int text_set_cursor(void *state, nk_gpu_dev_coordinate_t *location, uint32_t flags)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    
    DEBUG("text_set_cursor on %s\n", DEV_NAME(d));
    
    UNIMPL();
}
    
// graphics mode drawing commands

// confine drawing to this box overriding any previous boxes or regions
// a NULL clipping box should remove clipping limitations (reset to full screen size)
static int graphics_set_clipping_box(void *state, nk_gpu_dev_box_t *box)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    
    DEBUG("graphics_set_clipping_box on %s (%u, %u) (%u, %u)\n", DEV_NAME(d),
	  box->x,box->y,box->x+box->width, box->y+box->height);

    //
    // WRITE ME
    //

    if (box == NULL) {
        d->clipping_box = d->frame_box;
    } else {
        d->clipping_box = *box;
    }

    return 0;
}

// confine drawing to this region overriding any previous regions or boxes
// a NULL clipping region should remove clipping limitations (reset to full screen size)
static int graphics_set_clipping_region(void *state, nk_gpu_dev_region_t *region)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("graphics_set_clipping_region on %s\n", DEV_NAME(d));
    
    UNIMPL();
}

// Helper function:   is the coordinate within the box?
static inline int in_box(nk_gpu_dev_box_t *b, nk_gpu_dev_coordinate_t *c)
{
    return
	c->x>=b->x && c->x<(b->x+b->width) &&
	c->y>=b->y && c->y<(b->y+b->height);
}

// Helper function:  given a framebuffer, a box describing it, and a coordinate,
// return pointer to the pixel at the coordinate
static inline nk_gpu_dev_pixel_t* get_pixel_pointer(struct virtio_gpu_dev* d, uint32_t x, uint32_t y)
{
    return ((nk_gpu_dev_pixel_t*)d->frame_buffer) + y*d->frame_box.width + x;
}

// Get a pointer to a pixel within a bitmap
static inline nk_gpu_dev_pixel_t* get_bitmap_pixel_pointer(nk_gpu_dev_bitmap_t *bitmap, uint32_t x, uint32_t y)
{
    if (x >= bitmap->width || y >= bitmap->height) {
        return NULL;
    }
    return &(bitmap->pixels[x + (y*(bitmap->width))]);
}

// Helper functions for saturating arithmetic

static inline uint8_t saturating_add8(uint8_t a, uint8_t b) {
    uint8_t c = a + b;
    if (c < a) {
        c = UINT8_MAX;
    }
    return c;
}

static inline uint8_t saturating_sub8(uint8_t a, uint8_t b) {
    uint8_t c = a - b;
    if (c > a) {
        c = 0;
    }
    return c;
}

static inline uint8_t saturating_mul8(uint8_t a, uint8_t b) {
    // 16 bits can always fit the result
    uint16_t c = ((uint16_t) a) * ((uint16_t) b);
    if (c > UINT8_MAX) {
        c = UINT8_MAX;
    }
    return c;
}

static inline uint8_t saturating_div8(uint8_t a, uint8_t b) {
    if (b == 0) {
        return UINT8_MAX;
    }
    else {
        return a / b;
    }
}

// Helper function:  oldpixel = op(oldpixel,newpixel)
static void apply_with_blit(nk_gpu_dev_pixel_t *oldpixel, nk_gpu_dev_pixel_t *newpixel, nk_gpu_dev_bit_blit_op_t op)
{

    switch (op) {
        //
        // WRITE ME - other cases
        //
        case NK_GPU_DEV_BIT_BLIT_OP_COPY:
            oldpixel->raw = newpixel->raw;
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_NOT:
            oldpixel->raw = ~(oldpixel->raw);
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_AND:
            oldpixel->raw = (oldpixel->raw) & (newpixel->raw);
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_OR:
            oldpixel->raw = (oldpixel->raw) | (newpixel->raw);
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_NAND:
            oldpixel->raw = ~((oldpixel->raw) & (newpixel->raw));
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_NOR:
            oldpixel->raw = ~((oldpixel->raw) | (newpixel->raw));
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_XOR:
            oldpixel->raw = (oldpixel->raw) ^ (newpixel->raw);
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_XNOR:
            oldpixel->raw = ~((oldpixel->raw) ^ (newpixel->raw));
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_PLUS:
            for (uint8_t i = 0; i < 4; i++) {
                oldpixel->channel[i] = saturating_add8(oldpixel->channel[i],
                                                       newpixel->channel[i]);
            }
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_MINUS:
            for (uint8_t i = 0; i < 4; i++) {
                oldpixel->channel[i] = saturating_sub8(oldpixel->channel[i],
                                                       newpixel->channel[i]);
            }
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_MULTIPLY:
            for (uint8_t i = 0; i < 4; i++) {
                oldpixel->channel[i] = saturating_mul8(oldpixel->channel[i],
                                                       newpixel->channel[i]);
            }
            break;
        case NK_GPU_DEV_BIT_BLIT_OP_DIVIDE:
            for (uint8_t i = 0; i < 4; i++) {
                oldpixel->channel[i] = saturating_div8(oldpixel->channel[i],
                                                       newpixel->channel[i]);
            }
            break;
        default:
            oldpixel->raw = newpixel->raw;
            break;
    }

}

// Helper function:  oldpixel = op(oldpixel,newpixel) if in clipping box
// else does nothing
static inline void clip_apply_with_blit(void *state,
                                        nk_gpu_dev_coordinate_t *location,
                                        nk_gpu_dev_pixel_t *oldpixel,
                                        nk_gpu_dev_pixel_t *newpixel,
                                        nk_gpu_dev_bit_blit_op_t op) {

    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *) state;

    if (!in_box(&(d->clipping_box), location)) {
        return;
    } else {
        apply_with_blit(oldpixel, newpixel, op);
    }
}

static inline int graphics_draw_pixel(void *state, nk_gpu_dev_coordinate_t *location, nk_gpu_dev_pixel_t *pixel)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("graphics_draw_pixel 0x%08x on %s at (%u,%u)\n", pixel->raw, DEV_NAME(d),location->x, location->y);

    // location needs to be within the bounding box of the frame buffer
    // and pixel is only drawn if within the clipping box

    //
    // WRITE ME
    //

    clip_apply_with_blit(d,
                         location,
                         get_pixel_pointer(d, location->x, location->y),
                         pixel,
                         NK_GPU_DEV_BIT_BLIT_OP_COPY);
    return 0;
}

static inline int graphics_draw_line(void *state, nk_gpu_dev_coordinate_t *start, nk_gpu_dev_coordinate_t *end, nk_gpu_dev_pixel_t *pixel)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("graphics_draw_line 0x%x on %s from (%u,%u) to (%u,%u)\n", pixel->raw,
	  DEV_NAME(d),start->x,start->y, end->x, end->y);

    // line needs to be within the bounding box of the frame buffer
    // and only the portion of the line that is within the clipping box
    // is drawn

    //
    // WRITE ME
    //
    
    // Bresenham's line algorithm, adapted from
    // https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm#All_cases
    int x0 = start->x;
    int x1 = end->x;
    int y0 = start->y;
    int y1 = end->y;

    int dx = (x1 - x0 > 0) ? (x1 - x0) : (x0 - x1);
    int sx = x0 < x1 ? 1 : -1;
    int dy = - ((y1 - y0 > 0) ? (y1 - y0) : (y0 - y1));
    int sy = y0 < y1 ? 1 : -1;
    int error = dx + dy;
    
    while (1) {
        nk_gpu_dev_coordinate_t location = {.x = x0, .y = y0};
        graphics_draw_pixel(state, &location, pixel);
        if (x0 == x1 && y0 == y1) { break; }
        int e2 = 2 * error;
        if (e2 >= dy) {
            if (x0 == x1) { break; }
            error = error + dy;
            x0 = x0 + sx;
        }
        if (e2 <= dx) {
            if (y0 == y1) { break; }
            error = error + dx;
            y0 = y0 + sy;
        }
    }
    
    return 0;
}

static int graphics_draw_poly(void *state, nk_gpu_dev_coordinate_t *coord_list, uint32_t count, nk_gpu_dev_pixel_t *pixel)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    
    DEBUG("graphics_draw_poly on %s\n", DEV_NAME(d));

    // the poly needs to be within the bounding box of the frame buffer
    // and only the portion of the poly that is within the clipping box
    // is drawn

    //
    // WRITE ME
    //

    for (uint32_t i = 0; i < count; i++) {
        graphics_draw_line(state, &coord_list[i], &coord_list[(i + 1) % count], pixel);
    }

    return 0;
}

    
static int graphics_fill_box_with_pixel(void *state, nk_gpu_dev_box_t *box, nk_gpu_dev_pixel_t *pixel, nk_gpu_dev_bit_blit_op_t op)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("graphics_fill_box_with_pixel 0x%x on %s with (%u,%u) (%u,%u) op %d\n", pixel->raw,
	  DEV_NAME(d),box->x,box->y,box->x+box->width,box->y+box->height,op);

    // the filled box needs to be within the bounding box of the frame buffer
    // and only the portion of the box that is within the clipping box
    // is drawn

    //
    // WRITE ME
    //

    for (uint32_t i = 0; i < box->width; i++) {
        for (uint32_t j = 0; j < box->height; j++) {
            nk_gpu_dev_coordinate_t location = {.x = box->x + i, .y = box->y + j};
            clip_apply_with_blit(d,
                                 &location,
                                 get_pixel_pointer(d, location.x, location.y),
                                 pixel,
                                 op);
        }
    }

    return 0;
}

static int graphics_fill_box_with_bitmap(void *state, nk_gpu_dev_box_t *box, nk_gpu_dev_bitmap_t *bitmap, nk_gpu_dev_bit_blit_op_t op)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("graphics_fill_box_with_bitmap on %s\n", DEV_NAME(d));
    
    // copy from the bitmap to the framebuffer, using the op to transform (bitblt)
    // output pixels need to be within the bounding box of the frame buffer
    // and only the portion of that is within the clipping box is drawn

    //
    // WRITE ME
    //

    for (int i = 0; i < box->width; i++) {
        for (int j = 0; j < box->height; j++) {
            nk_gpu_dev_coordinate_t location = {.x = box->x + i, .y = box->y + j};
            clip_apply_with_blit(d,
                                 &location,
                                 get_pixel_pointer(d, location.x, location.y),
                                 get_bitmap_pixel_pointer(bitmap, i % bitmap->width, j % bitmap->height),
                                 op);
        }
    }

    return 0;
}

static int graphics_copy_box(void *state, nk_gpu_dev_box_t *source_box, nk_gpu_dev_box_t *dest_box, nk_gpu_dev_bit_blit_op_t op)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("graphics_copy_box on %s with (%u,%u) (%u,%u) to (%u, %u) (%u, %u) op %d\n",
	  DEV_NAME(d),source_box->x,source_box->y,source_box->x+source_box->width,
	  source_box->y+source_box->height,dest_box->x,dest_box->y,dest_box->x+dest_box->width,
	  dest_box->y+dest_box->height,op);

    // copy from one box in the framebuffer to another using the op to transform (bitblt) 
    // output pixels need to be within the bounding box of the frame buffer
    // and only the portion of that is within the clipping box is drawn

    //
    // WRITE ME
    //

    for (int i = 0; i < dest_box->width; i++) {
        for (int j = 0; j < dest_box->height; j++) {
            nk_gpu_dev_coordinate_t location = {.x = dest_box->x + i, dest_box->y + j};
            clip_apply_with_blit(d,
                                 &location,
                                 get_pixel_pointer(d, location.x, location.y),
                                 get_pixel_pointer(d,
                                                   source_box->x + (i % source_box->width),
                                                   source_box->y + (j % source_box->height)
                                 ),
                                 op);
        }
    }

    return 0;

}
    
static int graphics_draw_text(void *state, nk_gpu_dev_coordinate_t *location, nk_gpu_dev_font_t *font, char *string)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    
    DEBUG("graphics_draw_text on %s\n", DEV_NAME(d));

    //
    // EXTRA CREDIT
    //
    
    UNIMPL();
}


//  cursor functions, if supported
static int graphics_set_cursor_bitmap(void *state, nk_gpu_dev_bitmap_t *bitmap)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;

    DEBUG("graphics_set_cursor_bitmap on %s\n", DEV_NAME(d));

    //
    // EXTRA CREDIT
    //
    
    UNIMPL();
}

// the location is the position of the top-left pixel in the bitmap
static int graphics_set_cursor(void *state, nk_gpu_dev_coordinate_t *location)
{
    struct virtio_gpu_dev *d = (struct virtio_gpu_dev *)state;
    
    DEBUG("graphics_set_cursor on %s\n", DEV_NAME(d));

    //
    // EXTRA CREDIT
    //
    
    UNIMPL();
}



// mapping of interface callback functions to our implementations of them
static struct nk_gpu_dev_int ops = {
    .get_available_modes = get_available_modes,
    .get_mode = get_mode,
    .set_mode = set_mode,
    
    .flush = flush,
    
    .text_set_char = text_set_char,
    .text_set_cursor = text_set_cursor,

    .graphics_set_clipping_box = graphics_set_clipping_box,
    .graphics_set_clipping_region = graphics_set_clipping_region,

    .graphics_draw_pixel = graphics_draw_pixel,
    .graphics_draw_line = graphics_draw_line,
    .graphics_draw_poly = graphics_draw_poly,
    .graphics_fill_box_with_pixel = graphics_fill_box_with_pixel,
    .graphics_fill_box_with_bitmap = graphics_fill_box_with_bitmap,
    .graphics_copy_box = graphics_copy_box,
    .graphics_draw_text = graphics_draw_text,

    .graphics_set_cursor_bitmap = graphics_set_cursor_bitmap,
    .graphics_set_cursor = graphics_set_cursor,
};


////////////////////////////////////////////////////////
// Device initialization.
//
// In NK's virtio_pci framework, the framework discovers
// and does basic interogation of virtio devices.  Then,
// for each device, it invokes an initialization function,
// like this one.   The initialization function is responsble
// for device configuration and then registering the
// device to the rest of the kernel can use it.  The virtio_pci
// framework provides functions to do the common elements of this
int virtio_gpu_init(struct virtio_pci_dev *virtio_dev)
{
    char buf[DEV_NAME_LEN];
    
    DEBUG("initialize device\n");
    
    // allocate and zero a state structure for this device
    struct virtio_gpu_dev *dev = malloc(sizeof(*dev));
    if (!dev) {
	ERROR("cannot allocate state\n");
	return -1;
    }
    memset(dev,0,sizeof(*dev));

    // acknowledge to the device that we see it
    if (virtio_pci_ack_device(virtio_dev)) {
        ERROR("Could not acknowledge device\n");
        free(dev);
        return -1;
    }

    // ask the device for what features it supports
    if (virtio_pci_read_features(virtio_dev)) {
        ERROR("Unable to read device features\n");
        free(dev);
        return -1;
    }

    // tell the device what features we will support
    if (virtio_pci_write_features(virtio_dev, select_features(virtio_dev->feat_offered))) {
        ERROR("Unable to write device features\n");
        free(dev);
        return -1;
    }
    
    // initilize the device's virtqs.   The virtio-gpu device
    // has two of them.  The first is for most requests/responses,
    // while the second is for (mouse) cursor updates and movement
    if (virtio_pci_virtqueue_init(virtio_dev)) {
	ERROR("failed to initialize virtqueues\n");
	free(dev);
	return -1;
    }

    // associate our state with the general virtio-pci device structure,
    // and vice-versa:
    virtio_dev->state = dev;
    virtio_dev->teardown = teardown;    // the function we provide for deletion
    dev->virtio_dev = virtio_dev;

    // make sure our lock is in a known state
    spinlock_init(&dev->lock);
    
    
    // build a name for this device
    snprintf(buf,DEV_NAME_LEN,"virtio-gpu%u",__sync_fetch_and_add(&num_devs,1));
    
    // register the device, currently just as a generic device
    // note that this also creates an association with the generic
    // device represention elesewhere in the kernel
    dev->gpu_dev = nk_gpu_dev_register(buf,            // our name
				       0,               // no flags
				       &ops,            // our interface
				       dev);            // our state
    
    if (!dev->gpu_dev) {
	ERROR("failed to register block device\n");
	virtio_pci_virtqueue_deinit(virtio_dev);
	free(dev);
	return -1;
    }
    
    // Now we want to enable interrupts for the device
    // and register our handler
    //
    // This is MUCH more complicated than interrupt setup for
    // the parport device because the interrupt number or even
    // how many interrupts sources there are are not known beforehand
    // we have to figure it out as we boot.
    //
    // Also, interrupts for this device use a PCI technology called
    // MSI-X ("message signalled interrupts extended").  This setup code
    // is also similar to what would happen for a non-virtio PCI device
    // (see e1000e.c if you're curious)

    // if this is too terrifying you can shield your eyes until "device inited"

    // Note that this code will leak memory badly if interrupts cannot be configured

    // grab the pci device aspect of the virtio device
    struct pci_dev *pci_dev = virtio_dev->pci_dev;
    uint8_t i;
    ulong_t vec;
    
    if (virtio_dev->itype==VIRTIO_PCI_MSI_X_INTERRUPT) {
	// we assume MSI-X has been enabled on the device
	// already, that virtqueue setup is done, and
	// that queue i has been mapped to MSI-X table entry i
	// MSI-X is on but whole function is masked

	DEBUG("setting up interrupts via MSI-X\n");
	
	if (virtio_dev->num_virtqs != pci_dev->msix.size) {
	    DEBUG("weird mismatch: numqueues=%u msixsize=%u\n", virtio_dev->num_virtqs, pci_dev->msix.size);
	    // continue for now...
	    // return -1;
	}
	
	// this should really go by virtqueue, not entry
	// and ideally pulled into a per-queue setup routine
	// in virtio_pci...
	uint16_t num_vec = pci_dev->msix.size;
        
	// now fill out the device's MSI-X table
	for (i=0;i<num_vec;i++) {
	    // find a free vector
	    // note that prioritization here is your problem
	    if (idt_find_and_reserve_range(1,0,&vec)) {
		ERROR("cannot get vector...\n");
		return -1;
	    }
	    // register our handler for that vector
	    if (register_int_handler(vec, interrupt_handler, dev)) {
		ERROR("failed to register int handler\n");
		return -1;
	    }
	    // set the table entry to point to your handler
	    if (pci_dev_set_msi_x_entry(pci_dev,i,vec,0)) {
		ERROR("failed to set MSI-X entry\n");
		virtio_pci_virtqueue_deinit(virtio_dev);
		free(dev);
		return -1;
	    }
	    // and unmask it (device is still masked)
	    if (pci_dev_unmask_msi_x_entry(pci_dev,i)) {
		ERROR("failed to unmask entry\n");
		return -1;
	    }
	    DEBUG("finished setting up entry %d for vector %u on cpu 0\n",i,vec);
	}
	
	// unmask entire function
	if (pci_dev_unmask_msi_x_all(pci_dev)) {
	    ERROR("failed to unmask device\n");
	    return -1;
	}
	
    } else {
	ERROR("This device must operate with MSI-X\n");
	return -1;
    }

    DEBUG("device inited\n");
    return 0;
}

