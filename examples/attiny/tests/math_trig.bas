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
EMIT math.sin(0)
EMIT math.cos(0)
EMIT math.sin(8192)
EMIT math.cos(8192)
EMIT math.sin(16384)
EMIT math.cos(16384)
EMIT math.sin(-16384)
EMIT math.cos(-16384)
EMIT math.sin(12345)
EMIT math.cos(12345)
EMIT math.sin(-30000)
EMIT math.cos(-30000)
EMIT math.atan2(100, 100)
EMIT math.atan2(-300, 40)
EMIT math.atan2(5, -7)
EMIT math.atan2(0, -1)
EMIT math.atan2(32000, 1)
EMIT math.atan2(1, 32000)
EMIT math.deg_to_rad(30)
EMIT math.deg_to_rad(200)
EMIT math.deg_to_rad(-90)
EMIT math.rad_to_deg(16384)
EMIT math.rad_to_deg((-32767 - 1))
EMIT math.scale(1000, 16384)
EMIT math.scale(-1000, 32767)
EMIT math.scale(32767, 32767)
EMIT math.scale((-32767 - 1), (-32767 - 1))
EMIT math.scale((-32767 - 1), 32767)
EMIT math.scale(300, -8192)
END
