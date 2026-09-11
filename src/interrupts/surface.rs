use crate::interrupts::SoftInterruptFunction;

pub const NAME: &str = "surface";

#[allow(dead_code)]
pub static FUNCTIONS: [SoftInterruptFunction;11] = [
    SoftInterruptFunction { no:  0, name: "clear",          arguments: 1, returns: 0, help: "Clear surface (color)->()" },
    SoftInterruptFunction { no:  1, name: "draw_pixel",     arguments: 3, returns: 0, help: "Draw pixel (x,y,color)->()" },
    SoftInterruptFunction { no:  2, name: "draw_rect",      arguments: 5, returns: 0, help: "Draw rectagle (x,y,w,h,color)->()" },
    SoftInterruptFunction { no:  3, name: "fill_rect",      arguments: 5, returns: 0, help: "Fill rectagle (x,y,w,h,color)->()" },
    SoftInterruptFunction { no:  4, name: "draw_line",      arguments: 5, returns: 0, help: "Draw line (x1,y1,x2,y2,color)->()" },
    SoftInterruptFunction { no:  5, name: "draw_border",    arguments: 5, returns: 0, help: "Draw border (x,y,w,h,border_idx)->()" },
    SoftInterruptFunction { no:  6, name: "draw_image",     arguments: 3, returns: 0, help: "Draw image (x,y,image_idx)->()" },
    SoftInterruptFunction { no: 16, name: "get_size",       arguments: 0, returns: 2, help: "Get surface size ()->(w,h)" },
    SoftInterruptFunction { no: 17, name: "get_image_size", arguments: 1, returns: 2, help: "Get image size (image_idx)->(w,h)" },
    SoftInterruptFunction { no: 18, name: "get_clip",       arguments: 0, returns: 4, help: "Get surface clipping area ()->(x,y,w,h)" },
    SoftInterruptFunction { no: 19, name: "set_clip",       arguments: 4, returns: 0, help: "Set surface clipping area (x,y,w,h)->()" }
];