use crate::prelude::*;
use crate::kernel::{sync::IRQLock, irq, chardev};
use self::portio::{ParportIO, io_delay};

use bitfield::bitfield;
use lazy_static::lazy_static;

mod portio;

make_logging_macros!("parport");

const PARPORT0_BASE: u16 = 0x378;
const PARPORT0_IRQ: u8 = 7;
const PARPORT0_NAME: &str = "parport0";

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
#[derive(Debug)]
pub struct Parport {
    dev: Option<chardev::Registration<Self>>,
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

// We implement `chardev::Chardev` so that `Parport` can
// be registered with Nautilus' character device subsytem.
// We get access to `chardev::Registration::try_new` from
// this `impl`.
impl chardev::Chardev for Parport {
    type State = State;

    fn status(parport: &Self::State) -> chardev::Status {
        if parport.lock().is_ready() {
            chardev::Status::ReadableAndWritable
        } else {
            chardev::Status::Busy
        }
    }

    fn read(parport: &Self::State) -> chardev::RwResult<u8> {
        debug!("read!");
        let mut parport = parport.lock();
        if !parport.is_ready() {
            debug!("Unable to read while device is busy.");
            return chardev::RwResult::WouldBlock;
        }
        parport.status = ParportStatus::Busy;

        // mark device as busy
        debug!("setting device as busy");
        let mut stat = parport.port.read_stat();
        stat.set_busy(false); // stat.busy = 0
        parport.port.write_stat(&stat);

        parport.wait_for_attached_device();

        // disable output drivers for reading so no fire happens
        let mut ctrl = parport.port.read_ctrl();
        ctrl.set_bidir_en(true); // active low to enable output
        parport.port.write_ctrl(&ctrl);

        chardev::RwResult::Ok(parport.port.read_data().data)

    }

    fn write(parport: &Self::State, data: u8) -> chardev::RwResult {
        debug!("write!");
        let mut parport = parport.lock();
        if !parport.is_ready() {
            debug!("Unable to write while device is busy.");
            return chardev::RwResult::WouldBlock;
        }
        parport.status = ParportStatus::Busy;

        // mark device as busy
        debug!("setting device as busy");
        let mut stat = parport.port.read_stat();
        stat.set_busy(false); // stat.busy = 0
        parport.port.write_stat(&stat);

        parport.wait_for_attached_device();

        // set device to output mode
        debug!("setting device to output mode");
        let mut ctrl = parport.port.read_ctrl();
        ctrl.set_bidir_en(false); // ctrl.bidir_en = 0
        parport.port.write_ctrl(&ctrl);

        // write data byte to data register
        debug!("writing data to device");
        parport.port.write_data(&DataReg { data });

        // strobe the attached printer
        debug!("strobing device");
        ctrl.set_strobe(false); // ctrl.strobe = 0
        parport.port.write_ctrl(&ctrl);
        ctrl.set_strobe(true); // ctrl.strobe = 1
        parport.port.write_ctrl(&ctrl);
        ctrl.set_strobe(false); // ctrl.strobe = 0
        parport.port.write_ctrl(&ctrl);

        chardev::RwResult::Ok(())

    }

    fn get_characteristics(_state: &Self::State) -> Result<chardev::Characteristics> {
        Ok(chardev::Characteristics{/*`Characteristics` currently has no fields*/})
    }
}

impl Parport {
    /// Create an unitialized, unregistered `Parport`.
    pub fn new(port: ParportIO) -> Arc<State> {
        Arc::new(IRQLock::new(Self {
            dev: None,
            irq: None,
            port,
            status: ParportStatus::Ready,
        }))
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

    /// Registers the parport with the character device subsytem.
    fn register_chardev(parport: &Arc<State>, name: &str) -> Result {
        // Get rid of the previous registration, if any.
        // This means that registering twice is safe (but useless).
        Parport::unregister_chardev(parport);

        // Do the registration.
        parport.lock().dev = Some(
            chardev::Registration::try_new(name, Arc::clone(parport))
                .inspect_err(|_| error!("Parport IRQ registration failed."))?,
        );

        Ok(())
    }

    /// Unregister the character device.
    fn unregister_chardev(parport: &Arc<State>) {
        // The character device is unregistered whenever the `chardev::Registration`
        // is dropped.
        parport.lock().dev.take();
    }

    /// Initializes the parport registers.
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
    PARPORT.lock().init();

    Parport::register_irq(&PARPORT, irq as u16)?;
    Parport::register_chardev(&PARPORT, name)?;

    vc_println!("Registered device {}.", PARPORT.lock().dev.as_ref().unwrap().name());

    Ok(())
}

fn discover_and_bringup_devices() -> Result {
    bringup_device(PARPORT0_NAME, PARPORT0_IRQ)
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
            Parport::unregister_chardev(&PARPORT);
            Ok(())
        },
        _ => {
            vc_println!("Usage: parport up | parport down");
            Err(-1)
        }
    }

});
