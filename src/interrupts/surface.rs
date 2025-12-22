#[allow(dead_code)]
pub const MAP: (&str, &str) = (
"surface", "
  0, clear,          1, 0,
  1, draw_pixel,     3, 0,
  2, draw_rect,      5, 0,
  3, fill_rect,      5, 0,
  4, draw_line,      5, 0,
  5, draw_border,    5, 0,
  6, draw_image,     3, 0,
 16, get_size,       0, 2,
 17, get_image_size, 1, 2,
 18, get_clip,       0, 4,
 19, set_clip,       4, 0,
");