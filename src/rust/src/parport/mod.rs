use bitfield::bitfield;
use core::{
    ffi::{c_int, c_void},
    ptr::null,
};

use crate::prelude::*;
use crate::kernel::bindings;
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
    pub fn new(dev: NkCharDev, port: ParportIO, irq: Irq) -> Result<Arc<IRQLock<Parport>>> {
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
                locked_p.irq.register(shared_p.clone(), interrupt_handler).inspect_err(|e| {
                    error!("Failed to register interrupt handler. Error code {e}.")
                })?;
            }
            locked_p
                .dev
                .register(shared_p.clone())
                .inspect_err(|e| error!("Failed to register chardev. Error code {e}."))?;
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

    pub fn write(&mut self, data: u8) -> Result {
        if !self.is_ready() {
            debug!("Unable to write while device is busy.");
            return Err(-1);
        }
        self.state = ParportStatus::Busy;

        // mark device as busy
        debug!("setting device as busy");
        let mut stat = self.port.read_stat();
        stat.set_busy(false); // stat.busy = 0
        self.port.write_stat(&stat);

        self.wait_for_attached_device();

        // set device to output mode
        debug!("setting device to output mode");
        let mut ctrl = self.port.read_ctrl();
        ctrl.set_bidir_en(false); // ctrl.bidir_en = 0
        self.port.write_ctrl(&ctrl);

        // write data byte to data register
        debug!("writing data to device");
        self.port.write_data(&DataReg { data });

        // strobe the attached printer
        debug!("strobing device");
        ctrl.set_strobe(false); // ctrl.strobe = 0
        self.port.write_ctrl(&ctrl);
        ctrl.set_strobe(true); // ctrl.strobe = 1
        self.port.write_ctrl(&ctrl);
        ctrl.set_strobe(false); // ctrl.strobe = 0
        self.port.write_ctrl(&ctrl);

        Ok(())
    }

    fn read(&mut self) -> Result<u8> {
        if !self.is_ready() {
            debug!("Unable to read while device is busy.");
            return Err(-1);
        }
        self.state = ParportStatus::Busy;

        // mark device as busy
        debug!("setting device as busy");
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

unsafe fn bringup_device(name: &str, port: u16, irq: u8) -> Result {
    let port = unsafe { ParportIO::new(port) };
    let irq = Irq::new(irq);
    let dev = NkCharDev::new(name);
    let parport = Parport::new(dev, port, irq)?;
    debug!("{}", &parport.lock().get_name());

    Ok(())
}

fn discover_and_bringup_devices() -> Result {
    unsafe {
        // PARPORT0_BASE and PARPORT0_IRQ are valid and correct
        bringup_device("parport0", PARPORT0_BASE, PARPORT0_IRQ)
            .inspect_err(|e| error!("Failed to bring up parport device. Error code {e}."))?;
    }

    Ok(())
}

register_shell_command!("parport", "parport", |_, _| {
    vc_println!("Initializing parport ...");
    discover_and_bringup_devices()
        .inspect(|_| vc_println!("Done."))
        .inspect_err(|_| vc_println!("Unable to bring up parport device!"))
        .as_error_code()
});

unsafe fn deref_locked_state<'a>(state: *mut c_void) -> &'a IRQLock<Parport> {
    // caller must guarantee `state`, and the object it points to, was not mutated
    //
    // caller must not drop the strong reference count of the containing `Arc` to 0 while
    // the returned reference exists
    let l = state as *const IRQLock<Parport>;
    unsafe { l.as_ref() }.unwrap()
}

pub unsafe extern "C" fn interrupt_handler(
    _excp: *mut bindings::excp_entry_t,
    _vec: bindings::excp_vec_t,
    state: *mut c_void,
) -> c_int {
    let p = unsafe { deref_locked_state(state) };
    let mut l = p.lock();
    l.set_ready();

    // IRQ_HANDLER_END
    unsafe {
        bindings::apic_do_eoi();
    }
    0
    // l falls out of scope here, releasing the lock and reenabling interrupts after
    // IRQ_HANDLER_END. Redundant, but should work correctly.
}