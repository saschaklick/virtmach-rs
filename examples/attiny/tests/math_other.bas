REM Math test, the results leave as nibbles on PB0-PB3, PB4 toggling for each (see simtest.c)
REQ math, time, periph
FOR P = 0 TO 4
  periph.gpio_setup(P, 1)
NEXT P
SUB EMIT(V)
  FOR I = 0 TO 3
    N = (V >> (12 - 4 * I)) AND 15
    periph.gpio_write_mask(0, 15, N)
    periph.gpio_toggle(4)
    time.wait_for(0, 2)
  NEXT I
END SUB
EMIT math.sqr(10000)
EMIT math.sqr(32767)
EMIT math.sqr(0)
EMIT math.sqr(2)
EMIT math.pow(3, 5)
EMIT math.pow(2, 14)
EMIT math.pow(7, 5)
EMIT math.pow(-2, 15)
EMIT math.pow(10, 4)
EMIT math.random(1, 6)
EMIT math.random(1, 6)
EMIT math.random(1, 6)
EMIT math.random(1, 6)
EMIT math.random(1, 6)
EMIT math.random(1, 6)
EMIT math.random(1, 6)
EMIT math.random(1, 6)
EMIT math.random(-1000, 1000)
EMIT math.random(-1000, 1000)
EMIT math.random(-1000, 1000)
EMIT math.random(-1000, 1000)
EMIT math.random(-1000, 1000)
EMIT math.random(-1000, 1000)
EMIT -21846
time.set_tick(0, 200)
HALT
EMIT -17477
END
