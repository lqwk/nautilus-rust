#include <nautilus/nautilus.h>
#include <nautilus/thread.h>
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

const uint32_t N = 2;

static const nk_keycode_t NoShiftNoCaps[] = {
    KEY_UNKNOWN, ASCII_ESC, '1', '2',   /* 0x00 - 0x03 */
    '3', '4', '5', '6',                 /* 0x04 - 0x07 */
    '7', '8', '9', '0',                 /* 0x08 - 0x0B */
    '-', '=', ASCII_BS, '\t',           /* 0x0C - 0x0F */
    'q', 'w', 'e', 'r',                 /* 0x10 - 0x13 */
    't', 'y', 'u', 'i',                 /* 0x14 - 0x17 */
    'o', 'p', '[', ']',                 /* 0x18 - 0x1B */
    '\r', KEY_LCTRL, 'a', 's',          /* 0x1C - 0x1F */
    'd', 'f', 'g', 'h',                 /* 0x20 - 0x23 */
    'j', 'k', 'l', ';',                 /* 0x24 - 0x27 */
    '\'', '`', KEY_LSHIFT, '\\',        /* 0x28 - 0x2B */
    'z', 'x', 'c', 'v',                 /* 0x2C - 0x2F */
    'b', 'n', 'm', ',',                 /* 0x30 - 0x33 */
    '.', '/', KEY_RSHIFT, KEY_PRINTSCRN, /* 0x34 - 0x37 */
    KEY_LALT, ' ', KEY_CAPSLOCK, KEY_F1, /* 0x38 - 0x3B */
    KEY_F2, KEY_F3, KEY_F4, KEY_F5,     /* 0x3C - 0x3F */
    KEY_F6, KEY_F7, KEY_F8, KEY_F9,     /* 0x40 - 0x43 */
    KEY_F10, KEY_NUMLOCK, KEY_SCRLOCK, KEY_KPHOME,  /* 0x44 - 0x47 */
    KEY_KPUP, KEY_KPPGUP, KEY_KPMINUS, KEY_KPLEFT,  /* 0x48 - 0x4B */
    KEY_KPCENTER, KEY_KPRIGHT, KEY_KPPLUS, KEY_KPEND,  /* 0x4C - 0x4F */
    KEY_KPDOWN, KEY_KPPGDN, KEY_KPINSERT, KEY_KPDEL,  /* 0x50 - 0x53 */
    KEY_SYSREQ, KEY_UNKNOWN, KEY_UNKNOWN, KEY_UNKNOWN,  /* 0x54 - 0x57 */
};

#define KB_KEY_RELEASE 0x80
nk_keycode_t simple_kbd_translate(nk_scancode_t scan, int* out)
{
  int release;
  const nk_keycode_t *table=0;
  nk_keycode_t cur;
  nk_keycode_t flag;
  

  release = scan & KB_KEY_RELEASE;
  scan &= ~KB_KEY_RELEASE;

  table = NoShiftNoCaps;
  
  cur = table[scan];
  *out = release;
  return cur;
}

void scancode_handler(nk_scancode_t scan, void *priv) {
    int release;
    nk_keycode_t key = simple_kbd_translate(scan, &release);
    if (release == 0) {
        doom_key_down(key);
    } else {
        doom_key_up(key);
    }
}

void input_handler() {
    struct nk_vc_ops ops;
    ops.raw_noqueue = scancode_handler;
    struct nk_virtual_console* vc = nk_create_vc("doom", RAW_NOQUEUE, 0x0f, &ops, 0);
    nk_switch_to_vc(vc);

    while (true);
}

int run_doom(struct nk_gpu_dev* d, nk_gpu_dev_box_t* box) {
    doom_init(0, NULL, 0);
    nk_gpu_dev_bitmap_t* bitmap = malloc((sizeof(nk_gpu_dev_bitmap_t) + 4 * N * N * SCREENWIDTH * SCREENHEIGHT));
    bitmap->width = N * SCREENWIDTH;
    bitmap->height = N * SCREENHEIGHT;
    while (true) {
        doom_update();
        nk_gpu_dev_pixel_t* framebuffer = doom_get_framebuffer(4 /* RGBA */);

        for (uint32_t y = 0; y < SCREENHEIGHT; y++) {
            for (uint32_t x = 0; x < SCREENWIDTH; x++) {
                nk_gpu_dev_pixel_t p = framebuffer[y * SCREENWIDTH + x];
                for (uint32_t dy = 0; dy < N; dy++) {
                    for (uint32_t dx = 0; dx < N; dx++) {
                        bitmap->pixels[(N * y + dy) * N * SCREENWIDTH + N * x + dx] = p;
                    }
                }
            }
        }

        nk_gpu_dev_graphics_fill_box_with_bitmap(d, box, bitmap, NK_GPU_DEV_BIT_BLIT_OP_COPY);
        nk_gpu_dev_flush(d);
    }

    free(bitmap);

    return 0;
}

static int handle_doom (char * buf, void * priv) {
    char* name = "virtio-gpu0";
    struct nk_gpu_dev *d;
    nk_gpu_dev_video_mode_t modes[MAX_MODES], prevmode, *curmode;
    uint32_t nummodes=MAX_MODES;

    nk_fs_lfs_attach("virtio-blk0", "rootfs", 0);

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
    nk_gpu_dev_box_t clipping_box = {.x = (curmode->width - N * SCREENWIDTH) / 2,
                                     .y = (curmode->height - N * SCREENHEIGHT) / 2,
                                     .width = N * SCREENWIDTH,
                                     .height = N * SCREENHEIGHT};
    nk_gpu_dev_graphics_set_clipping_box(d, &clipping_box);

    // Change default bindings to modern mapping
    doom_set_default_int("key_up",          DOOM_KEY_W);
    doom_set_default_int("key_down",        DOOM_KEY_S);
    doom_set_default_int("key_strafeleft",  DOOM_KEY_A);
    doom_set_default_int("key_straferight", DOOM_KEY_D);
    doom_set_default_int("key_use",         DOOM_KEY_E);
    doom_set_default_int("key_left",        DOOM_KEY_H);
    doom_set_default_int("key_right",       DOOM_KEY_L);
    doom_set_default_int("key_fire",        DOOM_KEY_SPACE);
    doom_set_default_int("mouse_move",      0); // Mouse will not move forward

    nk_thread_start(input_handler, NULL, NULL, 1, TSTACK_DEFAULT, 0, 1);
    run_doom(d, &clipping_box);

    return 0;
}


static struct shell_cmd_impl doom_impl = {
    .cmd      = "doom",
    .help_str = "doom",
    .handler  = handle_doom,
};
nk_register_shell_cmd(doom_impl);

