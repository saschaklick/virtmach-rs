REM Fades a led on pin 1 in and out and moves a servo on pin 2 from 1000 to 2000
REM microseconds, pin 0 is a plain gpio output next to them.
REM After END: F = 1000, SERVO = 50, NONE = 0
REQ pwm, gpio, time
F = pwm.setup(1, 1000)
SERVO = pwm.setup(2, 50)
NONE = pwm.setup(3, 0)
gpio.setup(0, 1)
gpio.high(0)
FOR I = 0 TO 10
  pwm.duty(1, I, 10)
  pwm.pulse(2, 1000 + I * 100)
  OK = time.wait_until(0, 200)
NEXT I
FOR I = 10 TO 0 STEP -1
  pwm.duty(1, I, 10)
  OK = time.wait_until(0, 200)
NEXT I
pwm.stop(2)
END
