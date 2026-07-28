use crate::{VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return "dummy";
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &[];
    }

    fn call(&mut self, _vm: &mut VirtMach) {        
    }

}
