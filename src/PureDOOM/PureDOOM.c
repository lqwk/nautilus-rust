#include <nautilus/nautilus.h>
#include <nautilus/dev.h>
#include <nautilus/gpudev.h>
#include <nautilus/fs.h>
#include <nautilus/timer.h>
#include <nautilus/shell.h>
#include <nautilus/libccompat.h>

#define ERROR(fmt, args...) ERROR_PRINT("doom: " fmt, ##args)
#define DEBUG(fmt, args...) DEBUG_PRINT("doom: " fmt, ##args)
#define INFO(fmt, args...) INFO_PRINT("doom: " fmt, ##args)

#define DOOM_IMPLEMENTATION 
#include "PureDOOM.h"

#define MAX_MODES 64

int run_doom(struct nk_gpu_dev* d, nk_gpu_dev_box_t* box) {
    doom_init(0, NULL, 0);
    nk_gpu_dev_bitmap_t* bitmap = malloc (sizeof (nk_gpu_dev_bitmap_t) + 4 * SCREENWIDTH * SCREENHEIGHT);
    bitmap->width = SCREENWIDTH;
    bitmap->height = SCREENHEIGHT;
    while (true) {
        doom_update();
        uint8_t* framebuffer = doom_get_framebuffer(4 /* RGBA */);
        memcpy(bitmap->pixels, framebuffer, 4 * SCREENWIDTH * SCREENHEIGHT);
        nk_gpu_dev_graphics_fill_box_with_bitmap(d, box, bitmap, NK_GPU_DEV_BIT_BLIT_OP_COPY);
        nk_gpu_dev_flush(d);
    }

    free(bitmap);

    return 0;
}

static int handle_doom (char * buf, void * priv) {
    nk_fs_lfs_attach("virtio-blk0", "rootfs", 0);

    char* name = "virtio-gpu0";
    struct nk_gpu_dev *d;
    nk_gpu_dev_video_mode_t modes[MAX_MODES], prevmode, *curmode;
    uint32_t nummodes=MAX_MODES;

    if (!(d=nk_gpu_dev_find(name))) { 
        nk_vc_printf("Can't find %s\n",name);
        return -1;
    }

    if (nk_gpu_dev_get_mode(d,&prevmode)) {
        nk_vc_printf("Can't get mode\n");
        return -1;
    }

    if (nk_gpu_dev_get_available_modes(d,modes,&nummodes)) {
        nk_vc_printf("Can't get available modes from %s\n",name);
        return -1;
    }

    uint32_t i, sel=-1;
    
    for (i=0;i<nummodes;i++) {
        if (modes[i].type==NK_GPU_DEV_MODE_TYPE_GRAPHICS_2D) {
            sel = i;
        }
    }
    if (sel==-1) {
        nk_vc_printf("No graphics mode available (huh?) !!!!\n");
        return -1;
    }

    if (nk_gpu_dev_set_mode(d,&modes[sel])) {
        nk_vc_printf("Failed to set graphics mode....\n");
        return -1;
    }

    curmode = &modes[sel];
    nk_gpu_dev_box_t clipping_box = {.x = (curmode->width - SCREENWIDTH) / 2,
                                     .y = (curmode->height - SCREENHEIGHT) / 2,
                                     .width = SCREENWIDTH,
                                     .height = SCREENHEIGHT};
    nk_gpu_dev_graphics_set_clipping_box(d, &clipping_box);

    run_doom(d, &clipping_box);

    return 0;
}


static struct shell_cmd_impl doom_impl = {
    .cmd      = "doom",
    .help_str = "doom",
    .handler  = handle_doom,
};
nk_register_shell_cmd(doom_impl);

