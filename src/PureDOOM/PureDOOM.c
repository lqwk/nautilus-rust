#include <nautilus/nautilus.h>
#include <nautilus/dev.h>
#include <nautilus/gpudev.h>
#include <nautilus/timer.h>
#include <nautilus/shell.h>
#include <nautilus/libccompat.h>

#define ERROR(fmt, args...) ERROR_PRINT("doom: " fmt, ##args)
#define DEBUG(fmt, args...) DEBUG_PRINT("doom: " fmt, ##args)
#define INFO(fmt, args...) INFO_PRINT("doom: " fmt, ##args)

#define DOOM_IMPLEMENTATION 
#include "PureDOOM.h"

#define MAX_MODES 64

#define PRINT_MODE(spec, count, m)					\
    nk_vc_printf(spec "mode %d: %s %u by %u, offsets %u %u %u %u, flags 0x%lx, mouse %u by %u\n", \
		 (count),						\
		 (m)->type==NK_GPU_DEV_MODE_TYPE_TEXT ? "text" :		\
		 (m)->type==NK_GPU_DEV_MODE_TYPE_GRAPHICS_2D ? "graphics (2d)" : "UNKNOWN", \
		 (m)->width, (m)->height,					\
		 (m)->channel_offset[0], (m)->channel_offset[1], (m)->channel_offset[2], (m)->channel_offset[3], \
		 (m)->flags,						\
		 (m)->mouse_cursor_width, (m)->mouse_cursor_height)

// Helper macro for checking the result of a call
#define CHECK(x) if (x) { nk_gpu_dev_set_mode(d,&prevmode); nk_vc_printf("failed to do " #x "\n"); return -1; }

int run_doom(struct nk_gpu_dev* d, nk_gpu_dev_box_t* box) {
    /*DEBUG("calling doom_init ...\n");*/
    doom_init(0, NULL, 0);
    nk_gpu_dev_bitmap_t* bitmap = malloc (sizeof (nk_gpu_dev_bitmap_t) + 4 * SCREENWIDTH * SCREENHEIGHT);
    bitmap->width = SCREENWIDTH;
    bitmap->height = SCREENHEIGHT;
    while (true) {
        /*DEBUG("calling doom_update ...\n");*/
        doom_update();
        /*DEBUG("getting the framebuffer\n");*/
        uint8_t* framebuffer = doom_get_framebuffer(4 /* RGBA */);
        /*for (int i = 0; i < 4 * SCREENWIDTH * SCREENHEIGHT; i++) {*/
            /*DEBUG("%u\n", framebuffer[i]);*/
        /*}*/
        memcpy(bitmap->pixels, framebuffer, 4 * SCREENWIDTH * SCREENHEIGHT);
        /*DEBUG("filling box with bitmap\n");*/
        nk_gpu_dev_graphics_fill_box_with_bitmap(d, box, bitmap, NK_GPU_DEV_BIT_BLIT_OP_COPY);
        /*DEBUG("flushing the screen\n");*/
        nk_gpu_dev_flush(d);
    }

    free(bitmap);

    return 0;
}

static int handle_doom (char * buf, void * priv)
{
    /*doom_init(0, NULL, 0);*/
    /*while(true) {*/
        /*doom_update();*/
    /*}*/
    char name[32];
    struct nk_gpu_dev *d;
    nk_gpu_dev_video_mode_t modes[MAX_MODES], prevmode, *curmode;
    uint32_t nummodes=MAX_MODES;

    if ((sscanf(buf,"doom %s",name)!=1)) { 
	nk_vc_printf("doom devname\n",buf);
	return -1;
    }
    
    if (!(d=nk_gpu_dev_find(name))) { 
        nk_vc_printf("Can't find %s\n",name);
        return -1;
    }

    if (nk_gpu_dev_get_mode(d,&prevmode)) {
	nk_vc_printf("Can't get mode\n");
	return -1;
    }

    PRINT_MODE("current ",0,&prevmode);
    
    if (nk_gpu_dev_get_available_modes(d,modes,&nummodes)) {
        nk_vc_printf("Can't get available modes from %s\n",name);
        return -1;
    }

    uint32_t i, sel=-1;
    
    nk_vc_printf("Available modes are:\n");
    for (i=0;i<nummodes;i++) {
	PRINT_MODE("",i,&modes[i]);
	if (modes[i].type==NK_GPU_DEV_MODE_TYPE_GRAPHICS_2D) {
	    sel = i;
	}
    }
    if (sel==-1) {
	nk_vc_printf("No graphics mode available (huh?) !!!!\n");
	return -1;
    } else {
	nk_vc_printf("Using first graphics mode (%u)\n",sel);
    }

    if (nk_gpu_dev_set_mode(d,&modes[sel])) {
	nk_vc_printf("Failed to set graphics mode....\n");
	return -1;
    }

    curmode = &modes[sel];

    // assume that clipping (if available) is set to whole screen


    // Add your own test code here. You can remove this default test code we wrote
    //
    // WRITE ME!

    //*** start of existing test ***

    // clip small border around screen
    nk_gpu_dev_box_t clipping_box = {.x = (curmode->width - SCREENWIDTH) / 2,
                                     .y = (curmode->height - SCREENHEIGHT) / 2,
                                     .width = SCREENWIDTH,
                                     .height = SCREENHEIGHT};
    nk_gpu_dev_graphics_set_clipping_box(d, &clipping_box);

    // flush the initial display
    CHECK(nk_gpu_dev_flush(d));
    nk_vc_printf("Initial flush of the screen\n");
    run_doom(d, &clipping_box);

    return 0;
}


static struct shell_cmd_impl doom_impl = {
    .cmd      = "doom",
    .help_str = "doom dev",
    .handler  = handle_doom,
};
nk_register_shell_cmd(doom_impl);

