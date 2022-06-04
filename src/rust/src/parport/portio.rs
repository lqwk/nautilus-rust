use super::{CtrlReg, DataReg, StatReg};
use x86_64::instructions::port::{PortRead, PortWrite};

pub struct ParportIO {
    data_port: u16,
    stat_port: u16,
    ctrl_port: u16,
}

impl ParportIO {
    pub unsafe fn new(base_port: u16) -> Self {
        Self {
            data_port: base_port,
            stat_port: base_port + 1,
            ctrl_port: base_port + 2,
        }
    }

    pub fn read_data(&mut self) -> DataReg {
        let data = unsafe { u8::read_from_port(self.data_port) };
        DataReg { data }
    }
    pub fn write_data(&mut self, d: &DataReg) {
        unsafe { u8::write_to_port(self.data_port, d.data) }
    }

    pub fn read_stat(&mut self) -> StatReg {
        StatReg(unsafe { u8::read_from_port(self.stat_port) })
    }
    pub fn write_stat(&mut self, s: &StatReg) {
        unsafe { u8::write_to_port(self.stat_port, s.0) }
    }

    pub fn read_ctrl(&mut self) -> CtrlReg {
        CtrlReg(unsafe { u8::read_from_port(self.ctrl_port) })
    }
    pub fn write_ctrl(&mut self, c: &CtrlReg) {
        unsafe { u8::write_to_port(self.ctrl_port, c.0) }
    }
}
