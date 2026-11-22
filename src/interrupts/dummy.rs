use crate::{RuntimeError, Storage, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "dummy";

pub const FUNCTIONS: [SoftInterruptFunction;0] = [];

#[derive(Clone, Copy)]
pub struct Interrupt {}

impl Interrupt {
    /// A dummy for a slot of interrupt_list!. Every call is another one, as the dummy has no size
    /// and so no memory to share, like Box::leak(Box::new(Interrupt {})) without the allocation.
    #[doc(hidden)]
    pub fn slot<'a>() -> &'a mut Interrupt {
        // SAFETY: a reference to a type without size only needs to be aligned and not null
        unsafe { &mut *core::ptr::NonNull::<Interrupt>::dangling().as_ptr() }
    }
}

impl <S: Storage> SoftInterrupt<S> for Interrupt {
    fn name(&self) -> &str {
        return NAME;
    }

    #[cfg(feature = "compile")]
    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach<S>) {  
        vm.error = RuntimeError::UnimplementedInterruptFunc;
    }

}
