use core::ffi::c_int;
use core::fmt::Error;

use bitfield::bitfield;

use crate::prelude::*;

use chardev::NkCharDev;
use irq::Irq;
use portio::ParportIO;

use self::{lock::IRQLock, portio::io_delay};

mod chardev;
mod irq;
mod lock;
mod portio;

const PARPORT0_BASE: u16 = 0x378;
const PARPORT0_IRQ: u8 = 7;

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
    dev: NkCharDev,
    port: ParportIO,
    irq: Irq,
    state: ParportStatus,
}

impl Parport {
    pub fn new(dev: NkCharDev, port: ParportIO, irq: Irq) -> Result<Arc<IRQLock<Parport>>, Error> {
        let p = Parport {
            dev,
            port,
            irq,
            state: ParportStatus::Ready,
        };

        let shared_p = Arc::new(IRQLock::new(p));

        {
            let mut locked_p = shared_p.lock();
            unsafe {
                locked_p.irq.register(shared_p.clone())?;
            }
            locked_p.dev.register(shared_p.clone())?;
            locked_p.init();
        }

        Ok(shared_p)
    }

    fn init(&mut self) {
        let mut ctrl = CtrlReg(0); // bidir = 0, which means we are in output mode
        ctrl.set_select(true); // attached device selected
        ctrl.set_init(true); // active low => 1 means we are not initializing it
        ctrl.set_irq_en(true); // interrupt if we get an ack on the line
        self.port.write_ctrl(&ctrl);
    }

    fn wait_for_attached_device(&mut self) {
        //let mut count = 0;
        loop {
            io_delay();
            let stat = self.port.read_stat();
            //count += 1;
            if stat.busy() {
                break;
            }
        }
    }

    pub fn write(&mut self, data: u8) -> Result<(), Error> {
        if !self.is_ready() {
            return Err(Error);
        }
        self.state = ParportStatus::Busy;

        // mark device as busy
        vc_println!("setting device as busy");
        let mut stat = self.port.read_stat();
        stat.set_busy(false); // stat.busy = 0
        self.port.write_stat(&stat);

        self.wait_for_attached_device();

        // set device to output mode
        vc_println!("setting device to output mode");
        let mut ctrl = self.port.read_ctrl();
        ctrl.set_bidir_en(false); // ctrl.bidir_en = 0
        self.port.write_ctrl(&ctrl);

        // write data byte to data register
        vc_println!("writing data to device");
        self.port.write_data(&DataReg { data });

        // strobe the attached printer
        vc_println!("strobing device");
        ctrl.set_strobe(false); // ctrl.strobe = 0
        self.port.write_ctrl(&ctrl);
        ctrl.set_strobe(true); // ctrl.strobe = 1
        self.port.write_ctrl(&ctrl);
        ctrl.set_strobe(false); // ctrl.strobe = 0
        self.port.write_ctrl(&ctrl);

        Ok(())
    }

    fn read(&mut self) -> Result<u8, Error> {
        if !self.is_ready() {
            return Err(Error);
        }
        self.state = ParportStatus::Busy;

        // mark device as busy
        vc_println!("setting device as busy");
        let mut stat = self.port.read_stat();
        stat.set_busy(false); // stat.busy = 0
        self.port.write_stat(&stat);

        self.wait_for_attached_device();

        // disable output drivers for reading so no fire happens
        let mut ctrl = self.port.read_ctrl();
        ctrl.set_bidir_en(true); // active low to enable output
        self.port.write_ctrl(&ctrl);

        Ok(self.port.read_data().data)
    }

    fn get_name(&self) -> String {
        self.dev.get_name()
    }

    fn is_ready(&mut self) -> bool {
        self.state == ParportStatus::Ready
    }

    fn set_ready(&mut self) {
        self.state = ParportStatus::Ready;

        let mut stat = self.port.read_stat();
        stat.set_busy(true);
        self.port.write_stat(&stat);

        self.dev.signal();
    }
}

unsafe fn bringup_device(name: &str, port: u16, irq: u8) -> Result<(), Error> {
    let port = unsafe { ParportIO::new(port) };
    let irq = Irq::new(irq);
    let dev = NkCharDev::new(name);
    let parport = Parport::new(dev, port, irq)?;
    vc_println!("{}", &parport.lock().get_name());

    Ok(())
}

fn discover_and_bringup_devices() -> Result<(), Error> {
    unsafe {
        // PARPORT0_BASE and PARPORT0_IRQ are valid and correct
        bringup_device("parport0", PARPORT0_BASE, PARPORT0_IRQ)?;
    }

    Ok(())
}

#[no_mangle]
pub extern "C" fn nk_parport_init() -> c_int {
    vc_println!("partport init");
    if discover_and_bringup_devices().is_err() {
        -1
    } else {
        0
    }
}

register_shell_command!("parport", "parport", |_, _| {
    nk_parport_init();
});
