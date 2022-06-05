use super::{CtrlReg, DataReg, StatReg};
use x86_64::instructions::port::{PortRead, PortWrite};

const DELAY_PORT: u16 = 0x80;

pub struct ParportIO {
    pub data_port: u16,
    pub stat_port: u16,
    pub ctrl_port: u16,
}

impl ParportIO {
    pub unsafe fn new(base_port: u16) -> Self {
        Self {
            data_port: base_port,
            stat_port: base_port + 1,
            ctrl_port: base_port + 2,
        }
    }

    #[inline]
    pub fn read_data(&mut self) -> DataReg {
        let data = unsafe { u8::read_from_port(self.data_port) };
        DataReg { data }
    }
    #[inline]
    pub fn write_data(&mut self, d: &DataReg) {
        unsafe { u8::write_to_port(self.data_port, d.data) }
    }

    #[inline]
    pub fn read_stat(&mut self) -> StatReg {
        StatReg(unsafe { u8::read_from_port(self.stat_port) })
    }
    #[inline]
    pub fn write_stat(&mut self, s: &StatReg) {
        unsafe { u8::write_to_port(self.stat_port, s.0) }
    }

    #[inline]
    pub fn read_ctrl(&mut self) -> CtrlReg {
        CtrlReg(unsafe { u8::read_from_port(self.ctrl_port) })
    }
    #[inline]
    pub fn write_ctrl(&mut self, c: &CtrlReg) {
        unsafe { u8::write_to_port(self.ctrl_port, c.0) }
    }
}

#[inline]
pub fn io_delay() {
    unsafe { u8::write_to_port(DELAY_PORT, 0) };
}
