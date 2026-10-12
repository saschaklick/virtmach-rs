/*
 * Runs an ATtiny85 ELF in simavr, with things the simavr command line cannot do: it feeds UART
 * bytes into RX (PB0), plays an I2C device on SDA (PB3) and SCL (PB4) and decodes what the
 * firmware sends, so test.sh can check the results of the uart, i2c and math interrupts.
 *
 *   simtest FIRMWARE MILLISECONDS [options]
 *     --tx BAUD CONFIG            decode the UART TX on PB2, CONFIG as in uart.setup
 *     --rx BAUD CONFIG MS BYTE... send the bytes to RX from the time MS on
 *     --nibbles                   decode values sent as nibbles on PB0-PB3, PB4 toggling for each
 *     --i2c ADDRESS               an I2C device at ADDRESS with 8 registers, like SimI2c
 *     --hold-sda, --hold-scl      a device that holds the line low, for bus error and timeout
 *
 * Everything found is printed as a line, see the output() calls. Build with
 *   gcc -O2 -I/usr/local/include/simavr sim/simtest.c -o simtest -L/usr/local/lib -lsimavr -lm
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "sim_avr.h"
#include "sim_elf.h"
#include "avr_ioport.h"
#include "sim_cycle_timers.h"

#define DDRB 0x37
#define PORTB 0x38
#define MAX_EDGES 200000
#define MAX_EVENTS 20000

typedef struct { avr_cycle_count_t cycle; int level; } edge_t;

static avr_t *avr;
static avr_irq_t *pin[6];

/* --- edges of PB2, the UART TX, and the nibbles --- */
static edge_t tx_edges[MAX_EDGES];
static int tx_count;
static int pins_now[5];
static int nibbles[MAX_EDGES];
static avr_cycle_count_t nibble_cycle[MAX_EDGES];
static int nibble_count;
static int want_nibbles;

static void pin_changed(avr_irq_t *irq, uint32_t value, void *param)
{
	int n = (int)(long)param;
	value &= 1;
	if (n == 2 && tx_count < MAX_EDGES) {
		tx_edges[tx_count].cycle = avr->cycle;
		tx_edges[tx_count++].level = value;
	}
	if (n <= 4) {
		int before = pins_now[n];
		pins_now[n] = value;
		if (want_nibbles && n == 4 && before != value && nibble_count < MAX_EDGES) {
			nibbles[nibble_count] = pins_now[0] | pins_now[1] << 1 | pins_now[2] << 2 | pins_now[3] << 3;
			nibble_cycle[nibble_count++] = avr->cycle;
		}
	}
}

static double ms(avr_cycle_count_t cycle)
{
	return cycle * 1000.0 / avr->frequency;
}

/* --- UART RX, the bits are edges played by a cycle timer --- */
static edge_t rx_events[MAX_EVENTS];
static int rx_count, rx_next;

static avr_cycle_count_t rx_timer(avr_t *a, avr_cycle_count_t when, void *param)
{
	avr_raise_irq(pin[0], rx_events[rx_next].level);
	rx_next++;
	return rx_next < rx_count ? rx_events[rx_next].cycle : 0;
}

static void add_rx_bit(avr_cycle_count_t cycle, int level)
{
	if (rx_count && rx_events[rx_count - 1].level == level) return;
	rx_events[rx_count].cycle = cycle;
	rx_events[rx_count++].level = level;
}

static void rx_frames(double baud, int config, double start_ms, int count, int *bytes)
{
	double bit = avr->frequency / baud;
	int data_bits = 8 - (config & 3), parity = (config >> 2) & 3, stop_bits = 1 + ((config >> 4) & 1);
	double at = start_ms * avr->frequency / 1000.0;
	for (int i = 0; i < count; i++) {
		int ones = 0;
		add_rx_bit((avr_cycle_count_t)at, 0);
		at += bit;
		for (int b = 0; b < data_bits; b++) {
			int v = (bytes[i] >> b) & 1;
			ones += v;
			add_rx_bit((avr_cycle_count_t)at, v);
			at += bit;
		}
		if (parity) {
			add_rx_bit((avr_cycle_count_t)at, parity == 1 ? ones & 1 : !(ones & 1));
			at += bit;
		}
		add_rx_bit((avr_cycle_count_t)at, 1);
		at += (stop_bits + 1) * bit;
	}
}

/* --- the I2C device --- */
#define SDA_BIT 3
#define SCL_BIT 4
static int i2c_enabled, i2c_address, hold_sda, hold_scl;
static int regs[8];
static int ptr;
static int sda_slave_low;
static int bus_sda = 1, bus_scl = 1;
enum { IDLE, RX_ADDR, RX_DATA, TX_DATA, WAIT_STOP } mode = IDLE;
static int clk, shift, first_data, tx_byte, master_ack, pending;

static void load_tx(void)
{
	tx_byte = regs[ptr];
	ptr = (ptr + 1) % 8;
	sda_slave_low = !(tx_byte & 0x80);
}

static void i2c_rising(int sda)
{
	clk++;
	if (clk <= 8 && (mode == RX_ADDR || mode == RX_DATA)) shift = (shift << 1) | sda;
	if (clk == 9 && mode == TX_DATA && !pending) {
		master_ack = !sda;
		printf("i2c read 0x%02x %s\n", tx_byte, master_ack ? "ack" : "nack");
	}
}

static void i2c_falling(void)
{
	if (mode == IDLE || mode == WAIT_STOP) return;
	if (mode == TX_DATA) {
		if (clk >= 1 && clk <= 7) sda_slave_low = !((tx_byte >> (7 - clk)) & 1);
		else if (clk == 8) sda_slave_low = 0;
		else if (clk == 9) {
			clk = 0;
			if (master_ack) load_tx(); else { mode = WAIT_STOP; sda_slave_low = 0; }
		}
		return;
	}
	if (clk == 8) {
		if (mode == RX_ADDR) {
			int match = (shift >> 1) == i2c_address;
			printf("i2c addr 0x%02x %c %s\n", shift >> 1, shift & 1 ? 'R' : 'W', match ? "ack" : "nack");
			if (!match) { mode = WAIT_STOP; return; }
			sda_slave_low = 1;
			/* the next mode starts after the clock of the acknowledge */
			pending = shift & 1 ? TX_DATA : RX_DATA;
			first_data = 1;
		} else {
			printf("i2c write 0x%02x\n", shift);
			if (first_data) ptr = shift % 8; else { regs[ptr] = shift; ptr = (ptr + 1) % 8; }
			first_data = 0;
			sda_slave_low = 1;
		}
	} else if (clk == 9) {
		sda_slave_low = 0;
		clk = 0;
		shift = 0;
		if (pending) {
			mode = pending;
			pending = 0;
			if (mode == TX_DATA) load_tx();
		}
	}
}

static void i2c_bus(int sda, int scl)
{
	if (scl && bus_scl && sda != bus_sda) {
		if (!sda) { printf("i2c start\n"); mode = RX_ADDR; clk = 0; shift = 0; pending = 0; sda_slave_low = 0; }
		else { printf("i2c stop\n"); mode = IDLE; sda_slave_low = 0; }
	} else if (scl && !bus_scl) {
		i2c_rising(sda);
	} else if (!scl && bus_scl) {
		i2c_falling();
	}
	bus_sda = sda;
	bus_scl = scl;
}

static void i2c_poll(void)
{
	int ddr = avr->data[DDRB], port = avr->data[PORTB];
	int master_sda_low = (ddr & (1 << SDA_BIT)) && !(port & (1 << SDA_BIT));
	int master_scl_low = (ddr & (1 << SCL_BIT)) && !(port & (1 << SCL_BIT));
	int sda = !(master_sda_low || sda_slave_low || hold_sda);
	int scl = !(master_scl_low || hold_scl);
	if (sda == bus_sda && scl == bus_scl) return;
	int psda = bus_sda, pscl = bus_scl;
	if (i2c_enabled) {
		i2c_bus(sda, scl);
		/* the device may have changed the data line in response */
		sda = !(master_sda_low || sda_slave_low || hold_sda);
		bus_sda = sda;
	} else {
		bus_sda = sda;
		bus_scl = scl;
	}
	if (sda != psda) avr_raise_irq(pin[SDA_BIT], sda);
	if (scl != pscl) avr_raise_irq(pin[SCL_BIT], scl);
}

/* --- decoding the UART TX after the run --- */
static int level_at(avr_cycle_count_t cycle)
{
	int level = 1;
	for (int i = 0; i < tx_count && tx_edges[i].cycle <= cycle; i++) level = tx_edges[i].level;
	return level;
}

static void decode_tx(double baud, int config)
{
	double bit = avr->frequency / baud;
	int data_bits = 8 - (config & 3), parity = (config >> 2) & 3, stop_bits = 1 + ((config >> 4) & 1);
	avr_cycle_count_t after = 0;
	for (int i = 1; i < tx_count; i++) {
		if (tx_edges[i].level != 0 || tx_edges[i - 1].level != 1 || tx_edges[i].cycle < after) continue;
		avr_cycle_count_t start = tx_edges[i].cycle;
		int value = 0, ones = 0, ok = 1;
		const char *error = "";
		int n = 0;
		for (int b = 0; b < data_bits; b++, n++) {
			int v = level_at(start + (avr_cycle_count_t)((n + 1.5) * bit));
			value |= v << b;
			ones += v;
		}
		n++;
		if (parity) {
			int v = level_at(start + (avr_cycle_count_t)((data_bits + 1.5) * bit));
			if (v != (parity == 1 ? ones & 1 : !(ones & 1))) { ok = 0; error = "parity"; }
			n++;
		}
		for (int s = 0; s < stop_bits; s++) {
			if (level_at(start + (avr_cycle_count_t)((1 + data_bits + (parity != 0) + s + 0.5) * bit)) != 1) { ok = 0; error = "stop"; }
		}
		if (ok) printf("tx 0x%02x\n", value);
		else printf("txerror %s 0x%02x\n", error, value);
		after = start + (avr_cycle_count_t)((1 + data_bits + (parity != 0) + stop_bits - 0.5) * bit);
	}
}

static void print_nibbles(void)
{
	for (int i = 0; i + 3 < nibble_count; i += 4) {
		int value = nibbles[i] << 12 | nibbles[i + 1] << 8 | nibbles[i + 2] << 4 | nibbles[i + 3];
		if (value >= 32768) value -= 65536;
		printf("value %d %.3f %.3f\n", value, ms(nibble_cycle[i]), ms(nibble_cycle[i + 3]));
	}
}

static int number(const char *s) { return (int)strtol(s, NULL, 0); }

int main(int argc, char **argv)
{
	if (argc < 3) { fprintf(stderr, "usage: simtest FIRMWARE MILLISECONDS [options], see the top of simtest.c\n"); return 2; }
	elf_firmware_t f = {{0}};
	if (elf_read_firmware(argv[1], &f)) { fprintf(stderr, "cannot read %s\n", argv[1]); return 2; }
	avr = avr_make_mcu_by_name("attiny85");
	if (!avr) { fprintf(stderr, "no attiny85\n"); return 2; }
	avr_init(avr);
	avr_load_firmware(avr, &f);
	avr->frequency = f.frequency ? f.frequency : 1000000;
	double run_ms = atof(argv[2]);
	double tx_baud = 0;
	int tx_config = 0;
	for (int n = 0; n < 6; n++) {
		pin[n] = avr_io_getirq(avr, AVR_IOCTL_IOPORT_GETIRQ('B'), n);
		avr_irq_register_notify(pin[n], pin_changed, (void *)(long)n);
	}
	for (int i = 3; i < argc; i++) {
		if (!strcmp(argv[i], "--tx") && i + 2 < argc) { tx_baud = atof(argv[i + 1]); tx_config = number(argv[i + 2]); i += 2; }
		else if (!strcmp(argv[i], "--rx") && i + 3 < argc) {
			double baud = atof(argv[i + 1]);
			int config = number(argv[i + 2]), bytes[1000], count = 0;
			double start = atof(argv[i + 3]);
			for (i += 4; i < argc && strncmp(argv[i], "--", 2); i++) bytes[count++] = number(argv[i]);
			i--;
			rx_frames(baud, config, start, count, bytes);
		}
		else if (!strcmp(argv[i], "--nibbles")) want_nibbles = 1;
		else if (!strcmp(argv[i], "--i2c") && i + 1 < argc) { i2c_enabled = 1; i2c_address = number(argv[++i]); }
		else if (!strcmp(argv[i], "--hold-sda")) hold_sda = 1;
		else if (!strcmp(argv[i], "--hold-scl")) hold_scl = 1;
		else { fprintf(stderr, "unknown option %s\n", argv[i]); return 2; }
	}
	/* the lines idle high: RX, and the I2C lines through their pull-ups */
	if (rx_count) {
		avr_raise_irq(pin[0], 1);
		avr_cycle_timer_register(avr, rx_events[0].cycle, rx_timer, NULL);
	}
	if (i2c_enabled || hold_sda || hold_scl) {
		avr_raise_irq(pin[SDA_BIT], 1);
		avr_raise_irq(pin[SCL_BIT], 1);
	}
	avr_cycle_count_t end = (avr_cycle_count_t)(run_ms * avr->frequency / 1000.0);
	while (avr->cycle < end) {
		int state = avr_run(avr);
		if (state == cpu_Done || state == cpu_Crashed) break;
		if (i2c_enabled || hold_sda || hold_scl) i2c_poll();
	}
	if (tx_baud) decode_tx(tx_baud, tx_config);
	if (want_nibbles) print_nibbles();
	return 0;
}
