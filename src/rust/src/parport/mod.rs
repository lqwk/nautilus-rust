use core::ffi::c_int;
use core::fmt::Error;

use alloc::string::String;
use bitfield::bitfield;

use crate::{nk_bindings, utils::print_to_vc};
use chardev::{nk_char_dev_register, NkCharDev};
use irq::Irq;
use portio::ParportIO;

pub mod nk_shell_cmd;

mod chardev;
mod irq;
mod portio;

const PARPORT0_BASE: u16 = 0x378;
const PARPORT0_IRQ: u8 = 7;

extern "C" {
    fn spin_lock_irq(lock: *mut nk_bindings::spinlock_t) -> u8;
    fn spin_unlock_irq(lock: *mut nk_bindings::spinlock_t, flags: u8);
}

bitfield! {
    pub struct StatReg(u8);
    reserved, _: 1, 0;
    irq, _: 2;
    err, _: 3;
    sel, _: 4;
    pout, _: 5;
    ack, _: 6;
    busy, set_busy: 7;
}

bitfield! {
    pub struct CtrlReg(u8);
    strobe, set_strobe : 0;     // attached device strobe line - alert device to data (0->1->0)
    autolf, set_autolf : 1;     // attached device autolf line - auomatically add linefeeds to carriage returns (if 1)
    init, set_init : 2;         // attached device init line - init attached device (if 0)
    select, set_select : 3;     // attached device select print/in
    irq_en, set_irq_en : 4;     // enable interrupt when ack line is asserted by attached device
    bidir_en, set_bidir_en : 5; // select transfer direction 0 => write to attached device
    reserved, _ : 7, 6;         // reserved
}

pub struct DataReg {
    data: u8,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ParportStatus {
    Ready,
    Busy,
}

pub struct Parport {
    dev: Option<NkCharDev>,
    port: ParportIO,
    irq: Irq,
    state: ParportStatus,
    spinlock: nk_bindings::spinlock_t,
    state_flags: core::ffi::c_uchar,
}

//unsafe impl Sync for Parport {}
//unsafe impl Send for Parport {}

impl Parport {
    pub fn new(port: ParportIO, irq: Irq, name: &str) -> Result<Self, Error> {
        Ok(Parport {
            dev: None,
            port: port,
            irq: irq,
            state: ParportStatus::Ready,
            spinlock: 0,
            state_flags: 0,
        })
    }

    fn lock(&mut self) {
        let lock_ptr = &mut self.spinlock;
        self.state_flags = unsafe { spin_lock_irq(lock_ptr) };
    }

    fn unlock(&mut self) {
        let lock_ptr = &mut self.spinlock;
        unsafe { spin_unlock_irq(lock_ptr, self.state_flags) };
    }

    pub fn write(&mut self, data: u8) -> Result<(), Error> {
        if !self.is_ready() {
            return Err(Error);
        }
        self.state = ParportStatus::Busy;

        // mark device as busy
        print_to_vc("setting device as busy\n");
        let mut stat = self.port.read_stat();
        stat.set_busy(false); // stat.busy = 0
        self.port.write_stat(&stat);

        // set device to output mode
        print_to_vc("setting device to output mode\n");
        let mut ctrl = self.port.read_ctrl();
        ctrl.set_bidir_en(false); // ctrl.bidir_en = 0
        self.port.write_ctrl(&ctrl);

        // write data byte to data register
        print_to_vc("writing data to device\n");
        self.port.write_data(&DataReg { data });

        // strobe the attached printer
        print_to_vc("strobing device\n");
        ctrl.set_strobe(false); // ctrl.strobe = 0
        self.port.write_ctrl(&ctrl);
        ctrl.set_strobe(true); // ctrl.strobe = 1
        self.port.write_ctrl(&ctrl);
        ctrl.set_strobe(false); // ctrl.strobe = 0
        self.port.write_ctrl(&ctrl);

        Ok(())
    }

    fn read(&mut self) -> u8 {
        unimplemented!()
    }

    fn get_name(&self) -> String {
        self.dev.as_ref().unwrap().get_name()
    }

    fn is_ready(&self) -> bool {
        self.state == ParportStatus::Ready
    }
}

fn discover_and_bringup_devices() -> Result<(), Error> {
    let name = "parport0";

    let port = unsafe { ParportIO::new(PARPORT0_BASE) };
    let irq = unsafe { Irq::new(PARPORT0_IRQ.into()) };

    Parport::new(port, irq, name)?;

    //let r = nk_char_dev_register(name, &mut parport).unwrap();

    // parport.dev =

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
