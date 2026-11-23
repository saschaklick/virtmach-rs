use virtmach::{ RuntimeError, VMAtom, VirtMach, color::Color as VmColor, interrupts::{ self, SoftInterrupt, SoftInterruptFunction } };

extern crate sdl2;
use sdl2::{ video::Window, pixels::Color, render::Canvas, rect::{ Rect, Point } };

/// The palette of text and images: entry 1 is the color of text, 0 its background. set_color changes them, the
/// palette has to outlive the surface, which is created for every frame. Drawing functions take colors directly.
pub const DEFAULT_PALETTE: [Color;2] = [Color::RGB(0, 0, 0), Color::RGB(255, 255, 255)];

pub struct IntSurface <'a> {    
    pub canvas: &'a mut Canvas<Window>,
    pub clip: [i32;4],
    pub palette: &'a mut [Color;2]
}

impl IntSurface <'_> {
    /// Makes a color atom the draw color. A transparent color (alpha 0) draws nothing, then this returns false.
    fn use_color(&mut self, atom: VMAtom) -> bool {
        let [r, g, b, a] = VmColor::to_rgba(atom);
        self.canvas.set_draw_color(Color::RGBA(r, g, b, a));
        a != 0
    }

    fn draw_pixel(&mut self, x: i32, y: i32, color: Color) {
        self.canvas.set_draw_color(color);
        let _ = self.canvas.draw_point(Point::from((x as i32, y as i32)));
    }
}

impl SoftInterrupt for IntSurface <'_> {
    fn name(&self) -> &str {
        return interrupts::surface::NAME;
    }

    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &interrupts::surface::FUNCTIONS;
    }
    
    fn call(&mut self, vm: &mut VirtMach) {                
        self.canvas.set_clip_rect(Rect::from((self.clip[0], self.clip[1], self.clip[2] as u32, self.clip[3] as u32)));
        
        let op = vm.stack_pop();        
        
        match op {
            0 => {
                let color = vm.stack_pop();
                if self.use_color(color) { self.canvas.clear(); }
            }     
            1 | 6 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();                
                match op {
                    1 => {
                        let color = vm.stack_pop();
                        if self.use_color(color) { let _ = self.canvas.draw_point(Point::from((x as i32, y as i32))); }
                    }
                    _ => {
                        let _image = vm.stack_pop();
                        let ink = self.palette[1];
                        for i in 0..5 { self.draw_pixel(x as i32 + i, y as i32 + i, ink); self.draw_pixel(x as i32 + 4 - i, y as i32 + i, ink); }
                    }
                }
            }    
            2 | 3 | 5 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();
                let w = vm.stack_pop();
                let h = vm.stack_pop();
                // the fifth argument of a border is an index, of a rectangle a color
                let argument = vm.stack_pop();
                let visible = op == 5 || self.use_color(argument);
                match op {
                    2 => { if visible { let _ = self.canvas.draw_rect(Rect::from((x as i32, y as i32, w as u32, h as u32))); } }
                    3 => { if visible { let _ = self.canvas.fill_rect(Rect::from((x as i32, y as i32, w as u32, h as u32))); } }
                    5 => {                        
                        for by in y..y + h {
                            let c = if by != y && by != y + h - 1 && ((by - y) % 2 == 0 || by < y + 2 || by >= y + h - 3) { 1usize } else { 0usize };
                            self.draw_pixel(x as i32, by as i32, self.palette[c]);
                            self.draw_pixel((x + w - 1) as i32, by as i32, self.palette[c]);
                        }
                        for bx in x..x + w {
                            let c = if bx != x && bx != x + w - 1 && ((bx - x) % 2 == 0 || bx < x + 2 || bx >= x + w - 3) { 1usize } else { 0usize };
                            self.draw_pixel(bx as i32, y as i32, self.palette[c]); self.draw_pixel(bx as i32, (y + h - 1) as i32, self.palette[c]);
                        }
                    }
                    _ => {}
                }
            }
            4 => {
                let x_0 = vm.stack_pop();
                let y_0 = vm.stack_pop();
                let x_1 = vm.stack_pop();
                let y_1 = vm.stack_pop();
                let color = vm.stack_pop();
                if self.use_color(color) {
                    let _ = self.canvas.draw_line(Point::from((x_0 as i32, y_0 as i32)), Point::from((x_1 as i32, y_1 as i32)));
                }
            }
            10 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();
                let _font = vm.stack_pop();
                let text = vm.stack_pop() as u8;
                let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
                let texture_creator = self.canvas.texture_creator();
                let font = ttf_context.load_font("examples/programs/4x3.otf", 4).unwrap();                
                let surface = font.render(vm.get_str(text)).blended(self.palette[1]).unwrap();
                let texture = texture_creator.create_texture_from_surface(&surface).unwrap();
                let _ = self.canvas.copy(&texture, None, Rect::new(x as i32, y as i32, surface.width(), surface.height()));
            }
            15 => {
                let _font = vm.stack_pop();
                let text = vm.stack_pop() as u8;
                let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();                
                let font = ttf_context.load_font("examples/programs/4x3.otf", 4).unwrap();                
                let size = font.size_of(vm.get_str(text)).unwrap();                                
                vm.stack_push(size.0 as VMAtom);
                vm.stack_push(size.1 as VMAtom);
            }             
            16 => {
                [self.canvas.viewport().w, self.canvas.viewport().h].iter().for_each(|v| { vm.stack_push(*v as VMAtom); });                
            }
            17 => {
                let _image = vm.stack_pop();
                [5, 5].iter().for_each(|v| { vm.stack_push(*v as VMAtom); });                
            }      
            18 => {
                self.clip.iter().for_each(|v| vm.stack_push(*v as VMAtom));
            }      
            19 => {
                self.clip = [0i32;4].map(|_| { vm.stack_pop() as i32 });
            }                                   
            20 => {
                let (index, color) = (vm.stack_pop(), VmColor::to_rgba(vm.stack_pop()));
                match index {
                    0 | 1 => self.palette[index as usize] = Color::RGBA(color[0], color[1], color[2], color[3]),
                    _ => vm.error = RuntimeError::InterruptError
                }
            }
            21 => {
                let (r, g, b) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop());
                vm.stack_push(VmColor::from_rgb(r, g, b));
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }        
    }

}