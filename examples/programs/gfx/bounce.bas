REM Bouncing box inside a frame, uses the surface interrupt
REM HALT pauses the vm once per frame
REQ surface
W, H = surface.get_size()
surface.set_clip(0, 0, W, H)
X = 3 : Y = 5 : DX = 1 : DY = 1
WHILE 1
  surface.clear(0)
  surface.draw_rect(0, 0, W, H, 1)
  X = X + DX : Y = Y + DY
  IF X <= 1 OR X >= W - 2 THEN DX = -DX
  IF Y <= 1 OR Y >= H - 2 THEN DY = -DY
  surface.fill_rect(X - 1, Y - 1, 3, 3, 1)
  HALT
WEND
