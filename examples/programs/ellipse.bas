REM Rotating ellipse drawn with lines, uses the trig and surface interrupts
REM The points are RX * cos(A), RY * sin(A), turned by T every frame.
REM sin and cos return -MAX..MAX, trig.scale multiplies by them.
REM The radii are kept in 1/256 pixels and only the final point is rounded,
REM so the ellipse keeps its shape while it turns.
REM HALT pauses the vm once per frame
REQ surface, trig
W, H = surface.get_size()
surface.set_clip(0, 0, W, H)
CX = W / 2 : CY = H / 2
RX = 19 * 256 : RY = 9 * 256
T = 0
WHILE 1
  surface.clear(0)
  C = trig.cos(T) : S = trig.sin(T)
  FOR D = 0 TO 360 STEP 10
    A = trig.deg_to_rad(D)
    EX = trig.scale(RX, trig.cos(A)) : EY = trig.scale(RY, trig.sin(A))
    X = CX + ((trig.scale(EX, C) - trig.scale(EY, S) + 128) >> 8)
    Y = CY + ((trig.scale(EX, S) + trig.scale(EY, C) + 128) >> 8)
    IF D > 0 THEN surface.draw_line(PX, PY, X, Y, 1)
    PX = X : PY = Y
  NEXT D
  T = T + trig.deg_to_rad(3)
  HALT
WEND
