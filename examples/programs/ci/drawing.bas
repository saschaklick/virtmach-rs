REM CLS, PSET, PRESET, LINE, VIEW and COLOR on the surface interrupt, color index 1 draws and 0 is the background
DEFINT A-Z
REQ surface, math
COLOR #000000, #ffffff
CLS
PSET (10, 20)
PSET STEP (1, 2), 5
PRESET (3, 4)
LINE (0, 0)-(10, 5)
LINE -(20, 8), 7
LINE (5, 5)-STEP(10, 10), , B
LINE (30, 20)-(25, 25), 1, BF
VIEW (2, 3)-(11, 8)
VIEW
SCREEN 12
END
