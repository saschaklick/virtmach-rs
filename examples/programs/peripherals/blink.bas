REM Blinks a led on PB1 of the ATtiny85, one second on and one second off
REM time.wait_until keeps the period, whatever the loop does in between
REQ periph, time
LED = 1
periph.gpio_setup(LED, 1)
WHILE 1
  periph.gpio_toggle(LED)
  OK = time.wait_until(1, 0)
WEND
