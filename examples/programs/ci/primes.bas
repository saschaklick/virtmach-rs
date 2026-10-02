REM Counts the primes below LIMIT by trial division
REM After END: COUNT = 15, LAST = 47
LIMIT = 50
COUNT = 0
FOR N = 2 TO LIMIT - 1
  ISPRIME = 1
  D = 2
  WHILE D * D <= N AND ISPRIME
    IF N MOD D = 0 THEN ISPRIME = 0
    D = D + 1
  WEND
  IF ISPRIME THEN
    COUNT = COUNT + 1
    LAST = N
  END IF
NEXT N
END
