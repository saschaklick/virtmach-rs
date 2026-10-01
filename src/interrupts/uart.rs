//! UART template for microcontrollers.
//!
//! Only the stack handling is done here: every function pops its arguments, pushes 0 for each
//! return value and sets RuntimeError::UnimplementedInterruptFunc. Copy this into the firmware
//! and replace the cases with calls into the embedded framework (HAL) of the target.
//!
//! port: number of the UART peripheral, 0 for the first one
//! baud: baud rate divided by 10, so 115200 is 11520 and fits into 16 bit atoms
//! config: bits 0-1 data bits (8 - n), bits 2-3 parity (0 none, 1 even, 2 odd), bit 4 two stop bits,
//!         so 0 is 8N1
//!
//! Reading takes at most max bytes of what has already been received and does not wait, all
//! functions that move bytes return how many were read or sent. write_available tells how many
//! bytes fit into the transmit buffer, 0 means the uart is busy. In memory each byte takes one
//! cell, starting at address. Strings are string indices like in the string interrupt, start is
//! 0-based.

use crate::{Storage, RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "uart";

pub const INDEX: u8 = 8;

pub const FUNCTIONS: [SoftInterruptFunction;9] = [
    SoftInterruptFunction { no:  0, name: "setup",     arguments: 3, returns: 0, help: "Set up a port, baud rate / 10, config 0 is 8N1 (port,baud,config)->()" },
    SoftInterruptFunction { no:  1, name: "available", arguments: 1, returns: 1, help: "Number of received bytes waiting to be read (port)->(count)" },
    SoftInterruptFunction { no:  2, name: "read",      arguments: 1, returns: 1, help: "Read one received byte, -1 if there is none (port)->(byte)" },
    SoftInterruptFunction { no:  3, name: "write",     arguments: 2, returns: 0, help: "Send one byte (port,byte)->()" },
    SoftInterruptFunction { no:  4, name: "write_available", arguments: 1, returns: 1, help: "Number of bytes that can be sent without waiting, 0 while busy (port)->(count)" },
    SoftInterruptFunction { no: 10, name: "read_str",  arguments: 3, returns: 1, help: "Read up to max received bytes into a string (port,dest_index,max)->(count)" },
    SoftInterruptFunction { no: 11, name: "read_mem",  arguments: 3, returns: 1, help: "Read up to max received bytes into memory, one per cell (port,address,max)->(count)" },
    SoftInterruptFunction { no: 12, name: "write_str", arguments: 4, returns: 1, help: "Send length bytes of a string from start (port,src_index,start,length)->(count)" },
    SoftInterruptFunction { no: 13, name: "write_mem", arguments: 3, returns: 1, help: "Send length bytes from memory, one per cell (port,address,length)->(count)" }
];

pub struct Interrupt {}

impl <S: Storage> SoftInterrupt<S> for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        match op {
            0 => { let (_port, _baud, _config) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            1 => { let _port = vm.stack_pop(); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            2 => { let _port = vm.stack_pop(); vm.stack_push(-1); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            3 => { let (_port, _byte) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            4 => { let _port = vm.stack_pop(); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            10 => { let (_port, _dest_index, _max) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            11 => { let (_port, _address, _max) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            12 => { let (_port, _src_index, _start, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            13 => { let (_port, _address, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}
