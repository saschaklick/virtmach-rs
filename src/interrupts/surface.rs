use crate::{RuntimeError, VirtMach, interrupts::{ SoftInterrupt, SoftInterruptFunction }};

pub const NAME: &str = "surface";

#[allow(dead_code)]
pub static FUNCTIONS: [SoftInterruptFunction;13] = [
    SoftInterruptFunction { no:  0, name: "clear",          arguments: 1, returns: 0, help: "Clear surface (color)->()" },
    SoftInterruptFunction { no:  1, name: "draw_pixel",     arguments: 3, returns: 0, help: "Draw pixel (x,y,color)->()" },
    SoftInterruptFunction { no:  2, name: "draw_rect",      arguments: 5, returns: 0, help: "Draw rectagle (x,y,w,h,color)->()" },
    SoftInterruptFunction { no:  3, name: "fill_rect",      arguments: 5, returns: 0, help: "Fill rectagle (x,y,w,h,color)->()" },
    SoftInterruptFunction { no:  4, name: "draw_line",      arguments: 5, returns: 0, help: "Draw line (x1,y1,x2,y2,color)->()" },
    SoftInterruptFunction { no:  5, name: "draw_border",    arguments: 5, returns: 0, help: "Draw border (x,y,w,h,border_idx)->()" },
    SoftInterruptFunction { no:  6, name: "draw_image",     arguments: 3, returns: 0, help: "Draw image (x,y,image_idx)->()" },
    SoftInterruptFunction { no: 10, name: "draw_text",      arguments: 4, returns: 0, help: "Draw text (x,y,font_idx,db_index)->()" },
    SoftInterruptFunction { no: 15, name: "get_text_size",  arguments: 2, returns: 2, help: "Get text size (font_idx,db_index)->(w,h)" },
    SoftInterruptFunction { no: 16, name: "get_size",       arguments: 0, returns: 2, help: "Get surface size ()->(w,h)" },
    SoftInterruptFunction { no: 17, name: "get_image_size", arguments: 1, returns: 2, help: "Get image size (image_idx)->(w,h)" },
    SoftInterruptFunction { no: 18, name: "get_clip",       arguments: 0, returns: 4, help: "Get surface clipping area ()->(x,y,w,h)" },
    SoftInterruptFunction { no: 19, name: "set_clip",       arguments: 4, returns: 0, help: "Set surface clipping area (x,y,w,h)->()" }
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
            0 => { let _color = vm.stack_pop(); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            1 => { let (_x, _y, _color) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            2 | 3 => { let (_x, _y, _w, _h, _color) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            4 => { let (_x1, _y1, _x2, _y2, _color) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            5 => { let (_x, _y, _w, _h, _border_idx) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            6 => { let (_x, _y, _image_idx) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            10 => { let (_x, _y, _font_idx, _db_index) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            15 => { let (_font_idx, _db_index) = (vm.stack_pop(), vm.stack_pop()); vm.stack_push(0); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            16 => { vm.stack_push(0); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            17 => { let _image_idx = vm.stack_pop(); vm.stack_push(0); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            18 => { vm.stack_push(0); vm.stack_push(0); vm.stack_push(0); vm.stack_push(0); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            19 => { let (_x, _y, _w, _h) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop(), vm.stack_pop()); vm.error = RuntimeError::UnimplementedInterruptFunc; }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}
