REM Pong playing against itself, uses the surface and random interrupts
REM Each paddle follows the ball once it crosses into its half, one pixel per frame.
REM Hits near a paddle's edge send the ball off steeper, which the other
REM paddle may not catch in time. Each side's score is shown at the top
REM center of its half, both scores start over once a side reaches 10.
REM STR$ turns the scores into dynamic strings, which need the vm's alloc feature.
REQ surface, random, string

W, H = surface.get_size()
surface.set_clip(0, 0, W, H)
PH = 5
LY = H / 2 - PH / 2 : RY = LY
SL = 0 : SR = 0
GOSUB 1000

WHILE 1
  REM move the ball, bounce off top and bottom
  BX = BX + DX : BY = BY + DY
  IF BY <= 0 THEN BY = 0 : DY = ABS(DY)
  IF BY >= H - 1 THEN BY = H - 1 : DY = -ABS(DY)

  REM the paddle the ball is heading for moves towards it once it is on its half
  IF DX < 0 AND BX < W / 2 THEN
    IF BY < LY + 2 AND LY > 0 THEN LY = LY - 1
    IF BY > LY + 2 AND LY < H - PH THEN LY = LY + 1
  END IF
  IF DX > 0 AND BX >= W / 2 THEN
    IF BY < RY + 2 AND RY > 0 THEN RY = RY - 1
    IF BY > RY + 2 AND RY < H - PH THEN RY = RY + 1
  END IF

  REM paddle hits, a miss scores for the other side
  IF DX < 0 AND BX = 3 THEN
    T = BY - LY
    IF T >= 0 AND T < PH THEN DX = 1 : GOSUB 2000
  END IF
  IF DX > 0 AND BX = W - 4 THEN
    T = BY - RY
    IF T >= 0 AND T < PH THEN DX = -1 : GOSUB 2000
  END IF
  IF BX < 0 THEN SR = SR + 1 : GOSUB 1000
  IF BX >= W THEN SL = SL + 1 : GOSUB 1000
  IF SL > 9 OR SR > 9 THEN SL = 0 : SR = 0

  REM draw the net, paddles, ball and the scores centered on each half
  surface.clear(0)
  FOR T = 0 TO H - 1 STEP 4
    surface.draw_line(W / 2, T, W / 2, T + 1, 1)
  NEXT T
  surface.fill_rect(2, LY, 1, PH, 1)
  surface.fill_rect(W - 3, RY, 1, PH, 1)
  surface.fill_rect(BX, BY, 1, 1, 1)
  L$ = STR$(SL) : R$ = STR$(SR)
  T, TH = surface.get_text_size(0, L$)
  surface.draw_text(W / 4 - T / 2, 1, 0, L$)
  T, TH = surface.get_text_size(0, R$)
  surface.draw_text(W * 3 / 4 - T / 2, 1, 0, R$)
  HALT
WEND

1000 REM serve from the middle in a random direction
1010 BX = W / 2 : BY = random.range(4, H - 5)
1020 DX = random.range(0, 1) * 2 - 1
1030 DY = random.range(0, 1) * 2 - 1
1040 RETURN

2000 REM bounce angle from where the 5 pixel paddle was hit, the centre keeps the angle
2010 IF T = 0 THEN DY = -2 ELSE IF T = 1 THEN DY = -1 ELSE IF T = 3 THEN DY = 1 ELSE IF T = 4 THEN DY = 2
2020 RETURN
