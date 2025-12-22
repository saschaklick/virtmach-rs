use crate::{VirtMach, interrupts::{ SoftInterrupt }};

#[allow(dead_code)]
pub const MAP: (&str, &str) = ( "dummy", "");

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return "dummy";
    }

    fn call(&mut self, _vm: &mut VirtMach) {        
    }

}
