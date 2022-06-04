use core::ffi::c_int;
use core::fmt::Error;

use alloc::string::String;
use bitfield::bitfield;

use crate::utils::print_to_vc;
use chardev::{nk_char_dev_register, NkCharDev};
use irq::Irq;
use portio::ParportIO;

pub mod nk_shell_cmd;

mod chardev;
mod irq;
mod portio;

const PARPORT0_BASE: u16 = 0x378;
const PARPORT0_IRQ: u8 = 7;

bitfield! {
  struct StatReg(u8);
  reserved, _: 1, 0;
  irq, _: 2;
  err, _: 3;
  sel, _: 4;
  pout, _: 5;
  ack, _: 6;
  busy, set_busy: 7;
}

bitfield! {
struct CtrlReg(u8);
    strobe, set_strobe : 0;     // attached device strobe line - alert device to data (0->1->0)
    autolf, set_autolf : 1;     // attached device autolf line - auomatically add linefeeds to carriage returns (if 1)
    init, set_init : 2;         // attached device init line - init attached device (if 0)
    select, set_select : 3;     // attached device select print/in
    irq_en, set_irq_en : 4;     // enable interrupt when ack line is asserted by attached device
    bidir_en, set_bidir_en : 5; // select transfer direction 0 => write to attached device
    reserved, _ : 7, 6;         // reserved
}

struct DataReg {
    data: u8,
}

enum ParportStatus {
    Ready,
    Busy,
}

// pub struct Parport<'a> {
// dev: Option<NkCharDev<'a>>,

pub struct Parport {
    dev: Option<NkCharDev>,
    port: ParportIO,
    irq: Irq,
    state: ParportStatus,
    // TODO: lock parport internally
}

//unsafe impl Sync for Parport {}
//unsafe impl Send for Parport {}

// impl<'a> Parport<'a> {

impl Parport {
    pub fn new(port: ParportIO, irq: Irq, name: &str) -> Result<Self, Error> {
        Ok(Parport {
            dev: None,
            port: port,
            irq: irq,
            state: ParportStatus::Ready,
        })
    }

    fn wait_for_attached_device(&self) {
        unimplemented!()
    }

    // don't use read_write here since the mutabilities of data are different

    fn write(&mut self, data: &mut u8) -> i32 {
        unimplemented!()
    }

    fn read(&mut self, data: &u8) -> i32 {
        unimplemented!()
    }

    fn status(&self) -> i32 {
        unimplemented!()
    }

    fn interupt_handler(&mut self) -> i32 {
        unimplemented!()
    }

    fn init(&self) -> i32 {
        unimplemented!()
    }

    fn get_name(self) -> String {
        unimplemented!()
    }

    fn is_ready(self) -> bool {
        unimplemented!()
    }
}

// TODO: macros
// TODO: static struct nk_char_dev_int interface

pub fn nk_parport_init() -> c_int {
    if discover_and_bringup_devices().is_ok() {
        0
    } else {
        1
    }
}

fn discover_and_bringup_devices() -> Result<(), Error> {
    let name = "parport0";

    unsafe {
        let mut parport = Parport::new(
            ParportIO::new(PARPORT0_BASE),
            Irq::new(PARPORT0_IRQ.into()),
            name,
        )
        .unwrap();

        let r = nk_char_dev_register(name, &mut parport).unwrap();

        // parport.dev =
    }

    Ok(())
}

#[no_mangle]
pub extern "C" fn nk_parport_init() -> c_int {
    print_to_vc("partport init\n");
    if discover_and_bringup_devices().is_err() {
        -1
    } else {
        0
    }
}
