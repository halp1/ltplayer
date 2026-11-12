# ltplayer hardware

KiCad 10 project, second board layout. Schematic and 4-layer layout are done
and pass ERC and DRC with schematic parity. Not yet fabricated.

![Front](docs/board-top.png)

| Sheet | Contents |
| --- | --- |
| Power | USB4105 USB-C (5.1k CC pull-downs, USBLC6 ESD), BQ24074 1S LiPo charger with power path (1 A charge, 1.5 A input), soft power latch, TLV75533 3.3 V, TPS628438 buck to 2.0 V feeding two TPS7A2018 1.8 V LDOs (digital `+1V8`, analog `+1V8A`), battery solder pads plus an optional side-entry JST-PH |
| MCU | STM32U575ZIT6Q (SMPS variant, 2.2 uH on VLXSMPS), 16 MHz crystal for USB DFU, decoupling, BOOT0 pull-down, NRST cap, battery and VBUS sense dividers, ten buttons |
| Peripherals | Hirose DM3AT microSD on SDMMC1 4-bit with 47k pull-ups, Newhaven NHD-2.4-240320AF-CSXP 2.4in IPS panel on a 40-pin FH12 on the back, 2N7002 backlight switch |
| Audio | Two CS43131 (I2C 0x30 and 0x31), shared 22.5792 MHz 1.8 V oscillator for MCLK, PCA9306 I2C shifter, TXS0102 shifters on I2S, three TMUX4827 output switches, SJ1-3524N 3.5 mm and PJ-4450 4.4 mm jacks |

## Pin map

Kept as close to the current firmware (`src/main.rs`) as possible.

| Function | Pin | Notes |
| --- | --- | --- |
| SDMMC1 D0 D1 D2 D3 CLK CMD | PC8 PC9 PC10 PC11 PC12 PD2 | unchanged |
| SD card detect | PF15 | low = card present |
| SPI1 SCK MOSI | PA5 PA7 | unchanged, panel MISO (SDO) not connected |
| TFT CS DC RST | PD14 PD15 PF12 | unchanged, 4-wire SPI strapped by IM0/IM1/IM2 |
| Backlight | PE11 | TIM1_CH2 PWM into Q301, high = on |
| Buttons up down left right | PE13 PE14 PA3 PA2 | front d-pad, active low with internal pull-up |
| Buttons play/pause back | PE10 PB0 | play/pause moved from PC3 so every button has its own EXTI line |
| Button menu | PE15 | active high, shares the BOOT0 net (see Flashing) |
| Buttons vol+ vol- | PE7 PE8 | right edge SKSC, active low with internal pull-up |
| Power button | PE12 | active high, VSYS through a 100k/200k divider |
| Power hold | PC13 | drive high at boot to keep the regulators on |
| SAI1 FS_A SCK_A SD_A | PE4 PE5 PE6 | |
| DAC INT | PE1 | moved from PE2, which shared an EXTI line with the right button |
| DAC reset control | PE3 | inverted through Q401: low releases reset, high or floating holds it |
| I2C1 SCL SDA | PB8 PB9 | |
| JACK_SEL | PG0 | low = 3.5 mm |
| Jack detect 3.5 mm, 4.4 mm | PG1, PG2 | |
| USB DM DP, VBUS sense | PA11 PA12, PA9 | |
| Battery sense | PA0 | VBAT / 2 |
| BOOT0 | PH3 | 10k to GND, the menu button pulls it to +3V3 |
| HSE | PH0 PH1 | 16 MHz crystal Y201 |

## Power

The slide switch is gone. SW101 pulls `PWR_EN` up from VSYS through D103, which
starts all three regulators. Firmware then drives PC13 high, which holds
`PWR_EN` through D104, so the board stays on after the button is released. To
power off, drive PC13 low. R107 pulls `PWR_EN` down so the board stays off
while the button is up. PE12 reads the button through a divider so it also
works as a normal input while running.

USB power also holds `PWR_EN` through R110 and D105, so a plugged-in board
stays on whatever the firmware does. If the user switches off while it is
plugged in, firmware should turn the screen off and sleep.

The BQ24074 runs the system from USB and charges the battery at the same
time (R103 890 ohm sets 1.0 A, R105 1.1k sets a 1.5 A input limit, R111 10k
stands in for a thermistor). The TPS628438 buck turns the battery into 2.0 V,
and both 1.8 V LDOs run from that, so they only drop 0.2 V instead of 2 V.

### Battery

| Output | Draw from the battery | For 118 h with 20% margin |
| --- | --- | --- |
| 3.5 mm, one DAC | about 20 mA | 2.9 Ah |
| 4.4 mm balanced, two DACs | about 34 mA | 4.8 Ah |

Screen off, 0.1 mW per channel into the headphones, from datasheet typicals.
The screen adds about 36 mA, so keep it mostly off. The DACs are most of the
draw (Cirrus Table 3-19). Balanced needs the bigger cell because it runs
both DACs. A 4.8 Ah pouch cell is about 8 mm thick in the 60 x 85 mm space
behind the board. At 1 A it charges in about 5.5 hours. Measure a real board
before buying cells in bulk.

Solder the battery wires to J103 on the back. If the cell comes with a JST-PH
plug, fit J102 instead (side entry, about 4.9 mm tall).

## Display

Newhaven [NHD-2.4-240320AF-CSXP](https://newhavendisplay.com/2-4-inch-ips-tft-without-touchscreen/):
2.4in IPS, 240x320, ST7789VI, mounted landscape with the picture centred on
the board. 4-wire SPI (IM0 low, IM1 and IM2 high on the board). The panel runs
from +3V3. Its four backlight LEDs run from VSYS through R307 (22 ohm),
switched on the low side by Q301 from PE11. Firmware should scale the PWM
duty with the battery reading on PA0 to keep brightness even.

The cable leaves the panel's left edge, drops through a 1.2 x 22.5 mm slot in
the board and plugs into J302 on the back. Thread it through before you fit
the panel. The front shell holds the panel with adhesive.

## Flashing

Flashing is USB only. The STM32's built-in USB DFU bootloader does the work,
so no probe or back plate removal is needed.

1. Hold the menu button and plug in USB. Menu pulls BOOT0 high, so the chip
   starts in its bootloader. Firmware can also jump there from a menu item.
2. Flash: `dfu-util -a 0 -s 0x08000000:leave -D ltplayer.bin`, or use
   STM32CubeProgrammer in USB mode.

There is no debug header. Use `defmt` logging over USB for debugging.

## Audio routing

JACK_SEL drives all three TMUX4827 SEL pins (low selects SxA, high selects SxB).

* Low: U406 passes DAC1 HPOUTA/HPOUTB to the 3.5 mm tip and ring. DAC1 runs in stereo.
* High: U407 passes DAC1 outputs to 4.4 mm L+ and L-, U408 passes DAC2 outputs to R+ and R-. Both DACs run in mono (differential) mode, set by the register sequence in datasheet section 5.12.

Both DAC resets are held low by Q401 until firmware drives PE3 low, and R448
pulls `DAC_RESET` to VSYS, so VP is up before reset releases as the datasheet
requires. HPREFA/HPREFB of both DACs and the jack sleeves share `HP_GND`, a
dedicated In2 trace that joins `GND` only at R453 (0 ohm).

## Board

67.0 x 94.6 mm, 6.15 mm corner radius, which leaves 0.2 mm per side in the
67.4 x 95 enclosure pocket. Four layers:

| Layer | Use |
| --- | --- |
| F.Cu | parts, signals, GND fill |
| In1.Cu | solid GND |
| In2.Cu | +3V3 with a +1V8 island under the DACs and level shifters (notched for the display connector), VSYS and HP_GND traces |
| B.Cu | signals, microSD, display and battery connectors, GND fill |

Rules are in `ltplayer.kicad_pro` and sized for JLCPCB 4-layer: 0.1 mm
track and space minimum, 0.127 mm clearance, 0.45/0.25 mm vias. The three
TMUX4827 DSBGA parts need 0.3/0.15 mm via-in-pad (filled and capped, POFV).
`ltplayer.kicad_dru` relaxes hole clearance inside the USB-C footprint only,
where GCT's land pattern puts the GND pads 0.194 mm from the plastic pegs.

## Mechanical

[ltplayer-mech.dxf](docs/ltplayer-mech.dxf) and
[ltplayer-mech.csv](docs/ltplayer-mech.csv) have the outline, every hole and
the body outlines of the edge parts. Origin at the board centre, +X right,
+Y up, viewed from the front (display side), millimetres.

| Feature | Position | Size |
| --- | --- | --- |
| Corner holes H101 to H104 | (±27.35, ±41.15) | 2.2 mm plated, 4.4 mm GND pad, M2. PCB mount and back plate screws share them |
| Display panel | x -32.41 to +27.50, y -4.30 to +38.50 | 59.91 x 42.80 x 2.55 |
| Picture area | x -24.48 to +24.48, y -1.26 to +35.46 | 48.96 x 36.72, the window, centred |
| Display cable slot | x -31.81 to -30.61, y +6.35 to +27.85 | through the board, rounded ends |
| 3.5 mm jack J401 | x = -15.0, top edge | board notch 4.4 wide, 1.4 deep |
| 4.4 mm jack J402 | x = +15.0, top edge | body overhangs the edge by 1.6 mm |
| USB-C J101 | x = 0, bottom edge, front | |
| microSD J301 | x = +13.5, bottom edge, back | |
| Power, vol+, vol- | x = +31.15, y = +31.5, +17.5, +7.5 | right edge, side push |
| D-pad | centre (0, -24), up (0, -15.5), down (0, -32.5), left (-8.5, -24), right (+8.5, -24) | KMR2, play/pause in the middle |
| Back, menu | (-19, -10), (+19, -10) | KMR2 |
| Battery pads J103, JST-PH J102 | (-27.0, -30.0), (-27.5, -15.5) | back |
| Display connector J302 | (-25.6, +17.1) | back, opening faces the cable slot |

### 3D

`3d/ltplayer-assembly.step` is the board with every part and the display
panel, drawn from its datasheet with the picture area and the folded cable
as separate solids. It uses the same origin as the DXF, with +Z toward the
user and the board's back face at Z = 0. `3d/ltplayer-board.step` is the
board alone. Both are build outputs and not in git. Run `scripts/route.sh`, or
only the last block of it, to make them.

| Item | Z (mm) |
| --- | --- |
| Back of the optional JST-PH battery connector | -4.94 |
| Board back face, front face | 0, 1.6 |
| KMR2 button tops | 3.5 |
| 3.5 mm jack body top | 7.09 |
| 4.4 mm jack body top (estimate) | 8.2 |
| Display panel back, glass front | 8.5, 11.05 |

The panel sits 0.3 mm above the tallest part under it, which is the 4.4 mm
jack. That height is an estimate, so the panel height is too. Measure a real
jack, set `HEIGHT["J402"]` in `scripts/assembly.py`, and rerun it. If the jack
is under 6.3 mm tall the 3.5 mm jack takes over and the glass drops to 9.94.

Five parts have no model in KiCad's library and are plain blocks named
`placeholder`: the three side buttons (1.25 mm tall per Alps), the three
TMUX4827 (0.5 mm) and the 4.4 mm jack. KKG publishes no height for the
PJ-4450, so its 6.6 mm block is an estimate.

## Rebuilding the layout

The board is generated from the schematic by scripts. Hand edits in KiCad are
lost on the next run, so change the scripts instead.

```sh
scripts/route.sh   # 5 to 15 minutes
```

| Script | Step |
| --- | --- |
| `geom.py` | shared numbers: outline, DAC cells, plane split, net classes |
| `localize_fp.py` | copies edge-hanging footprints into `ltplayer.pretty` without edge and silk outlines |
| `build.py` | new board from the netlist, anchors the big parts, draws the outline |
| `place_passives.py` | places the rest next to the pin each one serves |
| `rules.py` | design rules and net classes |
| `plane_vias.py` | a via at every GND, +3V3, +1V8, VSYS and HP_GND pad |
| `dsn2l.py` | presents the board to Freerouting as two layers without the plane nets |
| `finish.sh` | imports the route, runs `trunk.py` (VSYS and HP_GND on In2), pours zones, removes leftovers, DRC |
| `mech.py` | the DXF and CSV above |

Needs Freerouting 2.5 and Java (`FREEROUTING` overrides the jar path) and the
KiCad Python at `~/code/pkg/mcp/kicad/venv`.

## Check before ordering

* **Display cable**: J302 pin 1 is the panel's GND. Check which side of the FH12 the cable's contacts face once it has gone through the slot, before ordering.
* **Display slot**: check the cable bends through the 1.2 mm slot without kinking, with a paper mock-up.
* **Charger heat**: 1 A from 5 V into a 3.7 V cell burns about 1.3 W in the BQ24074. It throttles itself if it gets hot, so the real charge time may be longer.
* **DFU clock**: AN2606 may let the STM32U575 bootloader run USB without a crystal. If it does, Y201 can stay unpopulated.
* **microSD**: the slot faces the bottom edge on the back. Confirm the card goes in where the enclosure leaves an opening.
* **4.4 mm jack**: the PJ-4450 pin assignment is estimated from the drawing. Confirm against the part.
* **Side buttons**: the SKSC peg hole spacing is scaled from the Alps drawing. Confirm against a part.
* **Via-in-pad**: confirm JLCPCB accepts 0.15 mm drills with POFV under the TMUX4827.
* **3.5 mm detect**: the SJ1-3524N tip switch reads through a 1M pull-up, so detect polarity flips against the 4.4 mm jack.
* **DAC RESET** comes from VSYS through R448. Check the VIH spec at low battery.

## Firmware changes

* Drive PC13 high early in boot to hold power, read the power button on PE12. On USB power, sleep instead of releasing PC13.
* PE3 drives reset through an inverter now: low releases both DACs.
* Buttons: up PE13, down PE14, play/pause PE10, vol+ PE7, vol- PE8 (active low). Menu PE15 is active high. DAC_INT moved to PE1.
* Display: landscape (320x240), ST7789VI on 4-wire SPI without MISO, backlight PWM on TIM1_CH2 (PE11) scaled with battery voltage.
* An "update" menu item that jumps to the system bootloader for USB DFU.
* SAI1 pins, the second DAC at 0x31, mono mode switching and the 3.5 mm detect polarity in `src/jack.rs`.
