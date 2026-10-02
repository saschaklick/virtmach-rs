REM Blinks a led on PB1 of the ATtiny85, one second on and one second off
REM time.wait_until keeps the period, whatever the loop does in between
REQ gpio, time
LED = 1
gpio.setup(LED, 1)
WHILE 1
  gpio.toggle(LED)
  OK = time.wait_until(1, 0)
WEND
