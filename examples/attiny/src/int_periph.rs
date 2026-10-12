//! The periph interrupt on the ATtiny85: gpio, pwm, uart and i2c in one interrupt, see
//! virtmach::interrupts::periph for the functions. It passes each function to the peripheral it
//! belongs to, counted from the first function of that peripheral. The peripherals the features
//! leave out are not there, their functions set RuntimeError::UnimplementedInterruptFunc.

use core::marker::PhantomData;
use virtmach::{ RuntimeError, Storage, VirtMach, interrupts::{ SoftInterrupt, periph } };

pub struct IntPeriph<'a> {
    #[cfg(feature = "gpio")]
    pub gpio: crate::int_gpio::IntGpio<'a>,
    #[cfg(feature = "pwm")]
    pub pwm: crate::int_pwm::IntPwm<'a>,
    #[cfg(any(feature = "uart_tx", feature = "uart_rx"))]
    pub uart: crate::int_uart::IntUart<'a>,
    #[cfg(feature = "i2c")]
    pub i2c: crate::int_i2c::IntI2c<'a>,
    pub _lifetime: PhantomData<&'a ()>
}

impl <S: Storage> SoftInterrupt<S> for IntPeriph<'_> {
    fn name(&self) -> &str {
        return periph::NAME;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {
        let op = vm.stack_pop();
        match op {
            #[cfg(feature = "gpio")]
            periph::GPIO..periph::PWM => self.gpio.call(vm, op - periph::GPIO),
            #[cfg(feature = "pwm")]
            periph::PWM..periph::UART => self.pwm.call(vm, op - periph::PWM),
            #[cfg(any(feature = "uart_tx", feature = "uart_rx"))]
            periph::UART..periph::I2C => self.uart.call(vm, op - periph::UART),
            #[cfg(feature = "i2c")]
            periph::I2C.. => self.i2c.call(vm, op - periph::I2C),
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}
