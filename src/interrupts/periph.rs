//! Peripheral templates for microcontrollers: GPIO, PWM, UART and I2C in one interrupt.
//!
//! Only the stack handling is done here: every function pops its arguments, pushes 0 for each
//! return value and sets RuntimeError::UnimplementedInterruptFunc. Copy this into the firmware
//! and replace the cases with calls into the embedded framework (HAL) of the target.
//!
//! The functions of each peripheral start at their own number and are named after it, so gpio
//! setup is `gpio_setup` at 20, pwm starts at 40, uart at 60 and i2c at 80. The numbers of a
//! peripheral are free from the next one, so it can grow.
//!
//! # GPIO (gpio_*, from 20)
//!
//! Pins are addressed by number. For the masked functions the pins are grouped into ports of
//! VMAtom::BITS pins, bit n of a mask on port p is pin p * VMAtom::BITS + n.
//!
//! mode: 0 input, 1 output push-pull, 2 output open-drain
//! pull: 0 none, 1 pull-up, 2 pull-down
//! level: 0 low, anything else high
//!
//! # PWM (pwm_*, from 40)
//!
//! Outputs are addressed by pin number like in gpio. Only some pins of a target can output PWM,
//! the others are an InterruptError. Pins often share a timer, then setting the frequency of one
//! changes it for the others too.
//!
//! setup makes the pin a PWM output with the frequency in Hz and returns the frequency the timer
//! actually runs at, or 0 if it cannot be reached, then the pin stays as it was. The duty cycle
//! starts at 0 until duty or pulse sets it. Frequencies only go up to VMAtom::MAX, 32767 Hz with
//! 16 bit atoms.
//!
//! duty sets the duty cycle to value / max, so it does not depend on the resolution of the timer
//! or the atom size: duty(pin, 1, 2) is 50%, duty(pin, 191, 255) is 75%. value is clamped to max,
//! max must be above 0. pulse sets the high time in microseconds instead, for servos that expect
//! 1000 to 2000 microseconds at 50 Hz, and is clamped to the period. Negative values, a max of 0
//! or a pin that was not set up are an InterruptError.
//!
//! stop drives the pin low and stops its output, setup starts it again.
//!
//! # UART (uart_*, from 60)
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
//!
//! # I2C controller (i2c_*, from 80)
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
//!

use crate::{Storage, RuntimeError, VirtMach, VMAtom, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "periph";

pub const INDEX: u8 = 8;

/// The number of the first function of each peripheral
pub const GPIO: VMAtom = 20;
pub const PWM: VMAtom = 40;
pub const UART: VMAtom = 60;
pub const I2C: VMAtom = 80;

pub const FUNCTIONS: [SoftInterruptFunction;33] = [
    SoftInterruptFunction { no: 20, name: "gpio_setup", arguments: 2, returns: 0, help: "Set the pin mode, 0 input, 1 output, 2 open-drain (pin,mode)->()" },
    SoftInterruptFunction { no: 21, name: "gpio_set_pull", arguments: 2, returns: 0, help: "Set the pull resistor, 0 none, 1 up, 2 down (pin,pull)->()" },
    SoftInterruptFunction { no: 22, name: "gpio_write", arguments: 2, returns: 0, help: "Set an output low or high (pin,level)->()" },
    SoftInterruptFunction { no: 23, name: "gpio_high", arguments: 1, returns: 0, help: "Set an output high (pin)->()" },
    SoftInterruptFunction { no: 24, name: "gpio_low", arguments: 1, returns: 0, help: "Set an output low (pin)->()" },
    SoftInterruptFunction { no: 25, name: "gpio_toggle", arguments: 1, returns: 0, help: "Toggle an output (pin)->()" },
    SoftInterruptFunction { no: 26, name: "gpio_read", arguments: 1, returns: 1, help: "Read an input, 0 low, 1 high (pin)->(level)" },
    SoftInterruptFunction { no: 30, name: "gpio_write_mask", arguments: 3, returns: 0, help: "Set the outputs of a port whose mask bits are set to the bits of values (port,mask,values)->()" },
    SoftInterruptFunction { no: 31, name: "gpio_read_mask", arguments: 2, returns: 1, help: "Read the inputs of a port whose mask bits are set, the other bits are 0 (port,mask)->(values)" },
    SoftInterruptFunction { no: 40, name: "pwm_setup", arguments: 2, returns: 1, help: "Start PWM on a pin at a frequency in Hz with duty 0, returns the frequency reached or 0 (pin,frequency)->(actual)" },
    SoftInterruptFunction { no: 41, name: "pwm_duty", arguments: 3, returns: 0, help: "Set the duty cycle to value/max (pin,value,max)->()" },
    SoftInterruptFunction { no: 42, name: "pwm_pulse", arguments: 2, returns: 0, help: "Set the high time in microseconds (pin,microseconds)->()" },
    SoftInterruptFunction { no: 43, name: "pwm_stop", arguments: 1, returns: 0, help: "Stop PWM on a pin and drive it low (pin)->()" },
    SoftInterruptFunction { no: 60, name: "uart_setup", arguments: 3, returns: 0, help: "Set up a port, baud rate / 10, config 0 is 8N1 (port,baud,config)->()" },
    SoftInterruptFunction { no: 61, name: "uart_available", arguments: 1, returns: 1, help: "Number of received bytes waiting to be read (port)->(count)" },
    SoftInterruptFunction { no: 62, name: "uart_read", arguments: 1, returns: 1, help: "Read one received byte, -1 if there is none (port)->(byte)" },
    SoftInterruptFunction { no: 63, name: "uart_write", arguments: 2, returns: 0, help: "Send one byte (port,byte)->()" },
    SoftInterruptFunction { no: 64, name: "uart_write_available", arguments: 1, returns: 1, help: "Number of bytes that can be sent without waiting, 0 while busy (port)->(count)" },
    SoftInterruptFunction { no: 70, name: "uart_read_str", arguments: 3, returns: 1, help: "Read up to max received bytes into a string (port,dest_index,max)->(count)" },
    SoftInterruptFunction { no: 71, name: "uart_read_mem", arguments: 3, returns: 1, help: "Read up to max received bytes into memory, one per cell (port,address,max)->(count)" },
    SoftInterruptFunction { no: 72, name: "uart_write_str", arguments: 4, returns: 1, help: "Send length bytes of a string from start (port,src_index,start,length)->(count)" },
    SoftInterruptFunction { no: 73, name: "uart_write_mem", arguments: 3, returns: 1, help: "Send length bytes from memory, one per cell (port,address,length)->(count)" },
    SoftInterruptFunction { no: 80, name: "i2c_setup", arguments: 2, returns: 0, help: "Set up a port as controller, bus clock in kHz (port,speed)->()" },
    SoftInterruptFunction { no: 81, name: "i2c_probe", arguments: 2, returns: 1, help: "Check if a device acknowledges its address, 0 or negative status (port,device)->(status)" },
    SoftInterruptFunction { no: 82, name: "i2c_write_byte", arguments: 3, returns: 1, help: "Send one byte to a device (port,device,byte)->(count)" },
    SoftInterruptFunction { no: 83, name: "i2c_read_byte", arguments: 2, returns: 1, help: "Read one byte from a device (port,device)->(byte)" },
    SoftInterruptFunction { no: 84, name: "i2c_write_reg", arguments: 4, returns: 1, help: "Write a byte to a device register (port,device,reg,byte)->(count)" },
    SoftInterruptFunction { no: 85, name: "i2c_read_reg", arguments: 3, returns: 1, help: "Read a byte from a device register (port,device,reg)->(byte)" },
    SoftInterruptFunction { no: 90, name: "i2c_write_mem", arguments: 4, returns: 1, help: "Send length bytes from memory, one per cell (port,device,address,length)->(count)" },
    SoftInterruptFunction { no: 91, name: "i2c_read_mem", arguments: 4, returns: 1, help: "Read length bytes into memory, one per cell (port,device,address,length)->(count)" },
    SoftInterruptFunction { no: 92, name: "i2c_write_str", arguments: 5, returns: 1, help: "Send length bytes of a string from start (port,device,src_index,start,length)->(count)" },
    SoftInterruptFunction { no: 93, name: "i2c_read_str", arguments: 4, returns: 1, help: "Read length bytes into a string (port,device,dest_index,length)->(count)" },
    SoftInterruptFunction { no: 94, name: "i2c_write_read_mem", arguments: 6, returns: 1, help: "Send bytes from memory, then read into memory after a repeated start (port,device,src_address,src_length,dest_address,dest_length)->(count read)" }
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
            20 => { let (_pin, _mode) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            21 => { let (_pin, _pull) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            22 => { let (_pin, _level) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            23 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            24 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            25 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            26 => { let _pin = vm.stack_pop(); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            30 => { let (_port, _mask, _values) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            31 => { let (_port, _mask) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            40 => { let (_pin, _frequency) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            41 => { let (_pin, _value, _max) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            42 => { let (_pin, _microseconds) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            43 => { let _pin = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            60 => { let (_port, _baud, _config) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            61 => { let _port = vm.stack_pop(); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            62 => { let _port = vm.stack_pop(); vm.stack_push(-1); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            63 => { let (_port, _byte) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            64 => { let _port = vm.stack_pop(); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            70 => { let (_port, _dest_index, _max) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            71 => { let (_port, _address, _max) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            72 => { let (_port, _src_index, _start, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            73 => { let (_port, _address, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            80 => { let (_port, _speed) = (vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            81 => { let (_port, _device) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            82 => { let (_port, _device, _byte) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            83 => { let (_port, _device) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            84 => { let (_port, _device, _reg, _byte) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            85 => { let (_port, _device, _reg) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            90 => { let (_port, _device, _address, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            91 => { let (_port, _device, _address, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            92 => { let (_port, _device, _src_index, _start, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            93 => { let (_port, _device, _dest_index, _length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            94 => {
                let (_port, _device, _src_address, _src_length, _dest_address, _dest_length) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop());
                vm.stack_push(0);
                vm.error = RuntimeError::UnimplementedInterruptFunc;
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}
