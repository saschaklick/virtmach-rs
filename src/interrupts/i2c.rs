//! I2C controller template for microcontrollers.
//!
//! Only the stack handling is done here: every function pops its arguments, pushes 0 for each
//! return value and sets RuntimeError::UnimplementedInterruptFunc. Copy this into the firmware
//! and replace the cases with calls into the embedded framework (HAL) of the target.
//!
//! port: number of the I2C peripheral, 0 for the first one
//! speed: bus clock in kHz, e.g. 100, 400 or 1000
//! device: 7 bit address of the device, without the read/write bit
//!
//! Transfers wait until they are done and return the number of bytes transferred, or a negative
//! status: -1 the device did not acknowledge its address, -2 a data byte was not acknowledged,
//! -3 bus error or arbitration lost, -4 timeout. read_byte and read_reg return the byte instead
//! of a count. In memory each byte takes one cell, starting at address. Strings are string indices
//! like in the string interrupt, start is 0-based. write_read_mem writes and then reads with a
//! repeated start in between, as needed to read registers of most devices.

use crate::{RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "i2c";

pub const FUNCTIONS: [SoftInterruptFunction;11] = [
    SoftInterruptFunction { no:  0, name: "setup",          arguments: 2, returns: 0, help: "Set up a port as controller, bus clock in kHz (port,speed)->()" },
    SoftInterruptFunction { no:  1, name: "probe",          arguments: 2, returns: 1, help: "Check if a device acknowledges its address, 0 or negative status (port,device)->(status)" },
    SoftInterruptFunction { no:  2, name: "write_byte",     arguments: 3, returns: 1, help: "Send one byte to a device (port,device,byte)->(count)" },
    SoftInterruptFunction { no:  3, name: "read_byte",      arguments: 2, returns: 1, help: "Read one byte from a device (port,device)->(byte)" },
    SoftInterruptFunction { no:  4, name: "write_reg",      arguments: 4, returns: 1, help: "Write a byte to a device register (port,device,reg,byte)->(count)" },
    SoftInterruptFunction { no:  5, name: "read_reg",       arguments: 3, returns: 1, help: "Read a byte from a device register (port,device,reg)->(byte)" },
    SoftInterruptFunction { no: 10, name: "write_mem",      arguments: 4, returns: 1, help: "Send length bytes from memory, one per cell (port,device,address,length)->(count)" },
    SoftInterruptFunction { no: 11, name: "read_mem",       arguments: 4, returns: 1, help: "Read length bytes into memory, one per cell (port,device,address,length)->(count)" },
    SoftInterruptFunction { no: 12, name: "write_str",      arguments: 5, returns: 1, help: "Send length bytes of a string from start (port,device,src_index,start,length)->(count)" },
    SoftInterruptFunction { no: 13, name: "read_str",       arguments: 4, returns: 1, help: "Read length bytes into a string (port,device,dest_index,length)->(count)" },
    SoftInterruptFunction { no: 14, name: "write_read_mem", arguments: 6, returns: 1, help: "Send bytes from memory, then read into memory after a repeated start (port,device,src_address,src_length,dest_address,dest_length)->(count read)" }
];

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach) {
        let op = vm.stack_pop();
        match op {
            0 => { let (_port, _speed) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            1 => { let (_port, _device) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            2 => { let (_port, _device, _byte) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            3 => { let (_port, _device) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            4 => { let (_port, _device, _reg, _byte) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            5 => { let (_port, _device, _reg) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            10 => { let (_port, _device, _address, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            11 => { let (_port, _device, _address, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            12 => { let (_port, _device, _src_index, _start, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            13 => { let (_port, _device, _dest_index, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            14 => {
                let (_port, _device, _src_address, _src_length, _dest_address, _dest_length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop());
                vm.stack_push(0);
                vm.error = RuntimeError::UnimplementedInterruptFunc;
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}
