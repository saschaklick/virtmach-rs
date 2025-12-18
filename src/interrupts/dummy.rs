use crate::{VirtMach, interrupts::{ SoftInterrupt, SoftInterruptDef }};

#[allow(dead_code)]
pub static DEF: SoftInterruptDef = SoftInterruptDef {
    name: "dummy",
    functions: &[]
};

pub struct Interrupt {}

impl SoftInterrupt for Interrupt {
    fn name(&self) -> &str {
        return "dummy";
    }

    fn call(&mut self, _vm: &mut VirtMach) {        
    }

}
