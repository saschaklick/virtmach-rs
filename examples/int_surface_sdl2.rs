use virtmach::{ VirtMach, VMAtom, RuntimeError, interrupts::{ SoftInterrupt } };

extern crate sdl2;
use sdl2::{ video::Window, pixels::Color, render::Canvas, rect::{ Rect, Point } };

pub struct IntSurface <'a> {    
    pub canvas: &'a mut Canvas<Window>,
    pub clip: [i32;4]
}

impl IntSurface <'_> {
    fn draw_pixel(&mut self, x: i32, y: i32, color: u8) {
        self.canvas.set_draw_color(COLORS[if color == 0 { 0 } else { 1 }]);
        let _ = self.canvas.draw_point(Point::from((x as i32, y as i32)));
    }
}

static COLORS: [Color;2] = [Color::RGB(0, 0, 0), Color::RGB(255, 255, 255)];

impl SoftInterrupt for IntSurface <'_> {
    fn name(&self) -> &str {
        return "surface";
    }
    
    fn call(&mut self, vm: &mut VirtMach) {                
        self.canvas.set_clip_rect(Rect::from((self.clip[0], self.clip[1], self.clip[2] as u32, self.clip[3] as u32)));
        
        let op = vm.stack_pop();        
        
        match op {
            0 => {
                let color = vm.stack_pop() as u8;  
                self.canvas.set_draw_color(COLORS[if color == 0 { 0 } else { 1 }]);
                self.canvas.clear();
            }     
            1 | 6 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();                
                match op {
                    1 => { self.draw_pixel(x as i32, y as i32, vm.stack_pop() as u8);                        
                    }
                    _ => {
                        let _image = vm.stack_pop();
                        for i in 0..5 { self.draw_pixel(x as i32 + i, y as i32 + i, 1 as u8); self.draw_pixel(x as i32 + 4 - i, y as i32 + i, 1 as u8); }
                    }
                }
            }    
            2 | 3 | 5 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();
                let w = vm.stack_pop();
                let h = vm.stack_pop();
                let color = vm.stack_pop();                                         
                self.canvas.set_draw_color(COLORS[if color == 0 { 0 } else { 1 }]);
                match op {
                    2 => { let _ = self.canvas.draw_rect(Rect::from((x as i32, y as i32, w as u32, h as u32))); }
                    3 => { let _ = self.canvas.fill_rect(Rect::from((x as i32, y as i32, w as u32, h as u32))); }
                    5 => {                        
                        for by in y..y + h {
                            let c = if by != y && by != y + h - 1 && ((by - y) % 2 == 0 || by < y + 2 || by >= y + h - 3) { 1 } else { 0 };
                            self.draw_pixel(x as i32, by as i32, c as u8);
                            self.draw_pixel((x + w - 1) as i32, by as i32, c as u8);
                        }
                        for bx in x..x + w {
                            let c = if bx != x && bx != x + w - 1 && ((bx - x) % 2 == 0 || bx < x + 2 || bx >= x + w - 3) { 1 } else { 0 };
                            self.draw_pixel(bx as i32, y as i32, c as u8); self.draw_pixel(bx as i32, (y + h - 1) as i32, c as u8);
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
                self.canvas.set_draw_color(COLORS[if color == 0 { 0 } else { 1 }]);
                let _ = self.canvas.draw_line(Point::from((x_0 as i32, y_0 as i32)), Point::from((x_1 as i32, y_1 as i32)));
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
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }        
    }

}