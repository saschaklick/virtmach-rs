REM Colors of drawing statements: a color literal, a constant defined as one, or a constant index of the default VGA palette
REM that is translated when compiling. PSET and LINE draw in the color of COLOR, PRESET and CLS in its background.
DEFINT A-Z
REQ surface, math
CONST GREEN = #00ff00
CONST RED = 4
PSET (1, 1), #ff0000
PSET (2, 1), GREEN
PSET (3, 1), RED
PSET (4, 1), RED + 1
LINE (0, 0)-(5, 5), 15
PSET (5, 1)
PRESET (6, 1)
COLOR #123456
PSET (7, 1)
COLOR , #654321
PRESET (8, 1)
CLS
END
