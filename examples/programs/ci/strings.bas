REM Strings: every text in double quotes becomes a #db entry of the listing,
REM its value in BASIC is the entry's index, which interrupt functions take.
REM Identical texts share one entry. Write "" for a quote inside a text.
REM After END: HELLO = 0, SAME = 0, OTHER = 1, LEN1 = 13, LEN2 = 12, LEN3 = 0
REQ surface, string
HELLO = "Hello, World!"
SAME = "Hello, World!"
OTHER = "Hi; ""quoted"""
LEN1 = string.get_length(HELLO)
LEN2 = string.get_length(OTHER)
LEN3 = string.get_length("")
surface.draw_text(1, 1, 0, HELLO)
IF HELLO = SAME THEN surface.draw_text(1, 10, 0, "same entry")
END
