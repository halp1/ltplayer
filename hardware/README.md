# ltplayer hardware

KiCad 10 project. Schematic first pass, no PCB layout yet.

| Sheet | Contents |
| --- | --- |
| Power | USB4105 USB-C (5.1k CC pull-downs, USBLC6 ESD), MCP73831 1S LiPo charger (500 mA via 2k PROG), VBUS/VBAT power path (B5819W + AO3401A), PCM12 slide switch on PWR_EN, AP2112K 3.3 V, two TPS7A2018 1.8 V LDOs (digital `+1V8`, analog `+1V8A`) |
| MCU | STM32U575ZIT6Q (SMPS variant, 2.2 uH on VLXSMPS), decoupling, BOOT0 pull-down, NRST cap, TC2030 SWD, battery and VBUS sense dividers, four buttons |
| Peripherals | Hirose DM3AT microSD on SDMMC1 4-bit with 47k pull-ups, EYESPI 18-pin FPC for the Adafruit 4311 |
| Audio | Two CS43131 (I2C 0x30 and 0x31), shared 22.5792 MHz 1.8 V oscillator for MCLK, PCA9306 I2C shifter, TXS0102 shifters on I2S, three TMUX4827 output switches, SJ1-3524N 3.5 mm and Pentaconn 4.4 mm jacks |

## Pin map

Kept as close to the current firmware (`src/main.rs`) as possible.

| Function | Pin | Notes |
| --- | --- | --- |
| SDMMC1 D0 D1 D2 D3 CLK CMD | PC8 PC9 PC10 PC11 PC12 PD2 | unchanged |
| SD card detect | PF15 | new, low = card present |
| SPI1 SCK MISO MOSI | PA5 PA6 PA7 | unchanged |
| TFT CS DC RST LITE | PD14 PD15 PF12 PE11 | unchanged |
| TFT SDCS TSCS MEMCS | PF14 PF13 PE9 | unchanged, held high |
| Buttons prev next select back | PA3 PA2 PC3 PB0 | unchanged, active low with internal pull-up |
| SAI1 FS_A SCK_A SD_A | PE4 PE5 PE6 | PE2 to PE6 block the firmware reserved |
| DAC INT, DAC RESET | PE2, PE3 | |
| I2C1 SCL SDA | PB8 PB9 | as reserved in the firmware |
| JACK_SEL | PG0 | unchanged, low = 3.5 mm |
| Jack detect 3.5 mm, 4.4 mm | PG1, PG2 | unchanged |
| USB DM DP, VBUS sense | PA11 PA12, PA9 | |
| Battery sense | PA0 | VBAT / 2 |
| BOOT0 | PH3 | 10k to GND |

## Audio routing

JACK_SEL drives all three TMUX4827 SEL pins (low selects SxA, high selects SxB).

* Low: U406 passes DAC1 HPOUTA/HPOUTB to the 3.5 mm tip and ring. DAC1 runs in stereo.
* High: U407 passes DAC1 outputs to 4.4 mm L+ and L-, U408 passes DAC2 outputs to R+ and R-. Both DACs run in mono (differential) mode, set by the register sequence in datasheet section 5.12.

HPREFA/HPREFB of both DACs and the jack sleeves share `HP_GND`, which joins `GND` through net tie NT401 so layout can return it to a single point.

## Open items before layout

* **Pentaconn jack**: pick the exact Nippon DICS part, then fix the J402 pin numbers and draw its footprint. Pin numbers are placeholders.
* **SKSCLBE010 footprint**: not in the KiCad library. Draw it from the Alps land pattern.
* **3.5 mm detect**: the SJ1-3524N tip switch reads through a 1M pull-up, so detect polarity flips relative to the 4.4 mm jack. Confirm and update `src/jack.rs`.
* **DAC RESET** is driven at 3.3 V into a VP-domain pin (VP = VSYS, at least 3.0 V). Check against the VIH spec at low battery.
* **Firmware**: SAI1 pins, the I2C address of the second DAC, and mono mode switching are not in `drivers/audio` yet.
