#!/bin/sh
# Cargo runner: flashes the ELF passed as $1 to an ATtiny85 via avrdude.
# Override the programmer with AVR_PROGRAMMER / AVR_PORT / AVR_BAUD, e.g.
#   AVR_PROGRAMMER=stk500v1 AVR_PORT=/dev/ttyUSB0 AVR_BAUD=19200 cargo run --release
set -e
PROGRAMMER="${AVR_PROGRAMMER:-usbasp}"
set -- -p t85 -c "$PROGRAMMER" -U "flash:w:$1:e"
[ -n "$AVR_PORT" ] && set -- "$@" -P "$AVR_PORT"
[ -n "$AVR_BAUD" ] && set -- "$@" -b "$AVR_BAUD"
exec avrdude "$@"
