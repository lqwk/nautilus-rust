use x86_64::instructions::port::{PortRead, PortWrite};

pub struct ParportIO {
    data_port: u16,
    stat_port: u16,
    ctrl_port: u16,
}

impl ParportIO {
    pub fn new(base_port: u16) -> Self {
        Self {
            data_port: base_port,
            stat_port: base_port + 1,
            ctrl_port: base_port + 2,
        }
    }

    pub fn read_data(&mut self) -> u8 {
        unsafe { u8::read_from_port(self.data_port) }
    }
    pub fn write_data(&mut self, data: u8) {
        unsafe { u8::write_to_port(self.data_port, data) }
    }


    pub fn read_stat(&mut self) -> u8 {
        unsafe { u8::read_from_port(self.stat_port) }
    }
    pub fn write_stat(&mut self, data: u8) {
        unsafe { u8::write_to_port(self.stat_port, data) }
    }


    pub fn read_ctrl(&mut self) -> u8 {
        unsafe { u8::read_from_port(self.ctrl_port) }
    }
    pub fn write_ctrl(&mut self, data: u8) {
        unsafe { u8::write_to_port(self.ctrl_port, data) }
    }
}
