REM Rotating ellipse drawn with lines, uses the math and surface interrupts
REM The points are RX * cos(A), RY * sin(A), turned by T every frame.
REM sin and cos return -MAX..MAX, math.scale multiplies by them.
REM The radii are kept in 1/256 pixels and only the final point is rounded,
REM so the ellipse keeps its shape while it turns.
REM HALT pauses the vm once per frame
REQ surface, math
W, H = surface.get_size()
surface.set_clip(0, 0, W, H)
CX = W / 2 : CY = H / 2
RX = 19 * 256 : RY = 9 * 256
T = 0
WHILE 1
  surface.clear(0)
  C = math.cos(T) : S = math.sin(T)
  FOR D = 0 TO 360 STEP 10
    A = math.deg_to_rad(D)
    EX = math.scale(RX, math.cos(A)) : EY = math.scale(RY, math.sin(A))
    X = CX + ((math.scale(EX, C) - math.scale(EY, S) + 128) >> 8)
    Y = CY + ((math.scale(EX, S) + math.scale(EY, C) + 128) >> 8)
    IF D > 0 THEN surface.draw_line(PX, PY, X, Y, 1)
    PX = X : PY = Y
  NEXT D
  T = T + math.deg_to_rad(3)
  HALT
WEND
