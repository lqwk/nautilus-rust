use crate::prelude::*;
use crate::kernel::{irq, sync::IRQLock, chardev::NkCharDev};
use self::portio::{ParportIO, io_delay};

use bitfield::bitfield;
use lazy_static::lazy_static;

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

/// The data associated with the parport.
pub struct Parport {
    dev: Option<NkCharDev>,
    irq: Option<irq::Registration<Self>>,
    port: ParportIO,
    status: ParportStatus,
}

/// A helpful type alias.
///
/// We need a lock around our `Parport` data
/// to allow for thread-safe interrior mutability, and it must be
/// an `IRQLock` to avoid deadlocks in the interrupt handler. Note
/// that in C, the lock would be a member of the parport data, and
/// not a guard around it.
type State = IRQLock<Parport>;

// We must implement `irq::Handler` in order to handle interrupts.
// Note that this does not actually register the interrupt handler.
// This `impl` gives us access to `irq::Registration::try_new` for
// `Arc<State>`, which will register the handler when called (if
// it succeeds).
impl irq::Handler for Parport {
    type State = State;

    fn handle_irq(parport: &Self::State) -> Result {
        debug!("setting to ready");
        parport.lock().set_ready();
        Ok(())
        // End-of-interrupt will automatically be done
        // after this return.
    }
}

impl Parport {
    /// Create an unitialized, unregistered `Parport`.
    pub fn new(port: ParportIO) -> Arc<State> {
        let parport = Arc::new(IRQLock::new(Self {
            dev: None,
            irq: None,
            port,
            status: ParportStatus::Ready,
        }));

        parport
    }

    /// Register the interrupt handler.
    fn register_irq(parport: &Arc<State>, int_vec: u16) -> Result {
        // Get rid of the previous registration, if any.
        // This means that registering twice is safe (but useless).
        Parport::unregister_irq(parport);

        // Do the registration.
        parport.lock().irq = Some(
            irq::Registration::try_new(int_vec, Arc::clone(parport))
                .inspect_err(|_| error!("Parport IRQ registration failed."))?,
        );

        Ok(())
    }

    /// Unregister the interrupt handler.
    fn unregister_irq(parport: &Arc<State>) {
        // The IRQ handler is unregistered whenever the `irq::Registration`
        // is dropped.
        parport.lock().irq.take();
    }

    fn register_chardev(parport: &Arc<State>, dev: NkCharDev) -> Result {
        parport
            .lock()
            .dev
            .insert(dev)
            .register(parport.clone())
            .inspect_err(|e| error!("Failed to register chardev. Error code {e}."))?;

        Ok(())
    }

    fn init(&mut self) {
        let mut ctrl = CtrlReg(0); // bidir = 0, which means we are in output mode
        ctrl.set_select(true); // attached device selected
        ctrl.set_init(true); // active low => 1 means we are not initializing it
        ctrl.set_irq_en(true); // interrupt if we get an ack on the line
        self.port.write_ctrl(&ctrl);
    }

    fn wait_for_attached_device(&mut self) {
        loop {
            // TODO: Use binding to C `io_delay`.
            io_delay();
            let stat = self.port.read_stat();
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
        self.status = ParportStatus::Busy;

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

    pub fn read(&mut self) -> Result<u8> {
        if !self.is_ready() {
            debug!("Unable to read while device is busy.");
            return Err(-1);
        }
        self.status = ParportStatus::Busy;

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

    pub fn is_ready(&mut self) -> bool {
        self.status == ParportStatus::Ready
    }

    fn set_ready(&mut self) {
        self.status = ParportStatus::Ready;

        let mut stat = self.port.read_stat();
        stat.set_busy(true);
        self.port.write_stat(&stat);

        if let Some(ref mut chardev) = self.dev {
            chardev.signal();
        }
    }
}

lazy_static! {
    // We keep the parport state in the static PARPORT, which is useful
    // in case we need to deregister it.
    pub static ref PARPORT: Arc<State> = Parport::new(ParportIO::new(PARPORT0_BASE));
}

fn bringup_device(name: &str, irq: u8) -> Result {
    let dev = NkCharDev::new(name);

    PARPORT.lock().init();

    Parport::register_irq(&PARPORT, irq as u16)?;
    Parport::register_chardev(&PARPORT, dev)?;

    debug!("{:?}", PARPORT.lock().dev.as_ref().map(|dev| dev.name.as_str()));

    Ok(())
}

fn discover_and_bringup_devices() -> Result {
    bringup_device("parport0", PARPORT0_IRQ)
        .inspect_err(|e| error!("Failed to bring up parport device. Error code {e}."))
}


register_shell_command!("parport", "parport up | parport down", |command| {
    match command {
        "parport up" => {
            discover_and_bringup_devices()
                .inspect_err(|_| vc_println!("Unable to bring up parport device!"))
        },
        "parport down" => {
            Parport::unregister_irq(&PARPORT);
            // TODO: unregister the character device for the parport.
            Ok(())
        },
        _ => {
            vc_println!("Usage: parport up | parport down");
            Err(-1)
        }
    }

});
