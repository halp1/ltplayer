# ltplayer firmware starter

Rust firmware for the **ST NUCLEO-U575ZI-Q** (STM32U575ZIT6Q, Cortex-M33), using Embassy and the onboard ST-LINK debugger. Firmware builds need no STM32CubeIDE, separate programmer, or bootloader. Building the host-side probe-rs tools may require your platform's native build tools (for example Xcode Command Line Tools on macOS).

The **Adafruit EYESPI breakout is a passive wiring adapter**, not another device to flash. Firmware runs on the Nucleo. A display and its controller-specific driver are still required to draw anything or read touch. This starter deliberately does not assume a display model.

## First build and flash

Install [Rust with rustup](https://rustup.rs/) if needed. **Read the connection and option-byte checklist below before running `cargo flash-board`**; then use these commands in this directory:

```sh
rustup target add thumbv8m.main-none-eabihf
cargo install probe-rs-tools --locked
cargo build --release --locked --bins --examples
probe-rs list
```

`cargo flash-board` is the project alias for `cargo run --release --locked --bin ltplayer`: it builds, programs internal flash, resets/runs the chip, and streams `defmt` logs over RTT. It **replaces existing firmware**. Press Ctrl-C to leave the monitor; no serial port is needed. Use `probe-rs download` below if you want to flash without attaching a monitor.

1. Connect a **USB data cable to the Nucleo's ST-LINK USB connector** (not its MCU/user USB connector). Keep the onboard ST-LINK connected to the MCU and use the board's normal USB-powered jumper setup.
2. Before first flashing, use STM32CubeProgrammer to **inspect** option bytes: this single-image starter assumes **TrustZone disabled (`TZEN=0`)**, boot from internal flash, and an unlocked development device. It is not a secure/non-secure TrustZone pair. Do not assume the board's current settings from its model number.
3. Run `probe-rs list`; the onboard ST-LINK should appear.
4. Run `cargo flash-board` (or simply `cargo run --release --locked`).
5. Expect a startup RTT message and **LD1 (green, PC7) blinking every half second**. No EYESPI wiring or display is required for this test.

**Security settings:** this project never changes option bytes, RDP, or TrustZone. If `TZEN=1`, stop and use an appropriate secure boot project or deliberately reconfigure a development board following ST's instructions. Changing security settings can erase flash or lock a device; do not blindly change RDP/TZEN to fix a connection error.

The toolchain configuration requests stable Rust, rustfmt, clippy, and the correct `thumbv8m.main-none-eabihf` target. This is an Armv8-M target, not the Cortex-M4/M7 `thumbv7em` target. Cargo dependencies are locked for reproducible resolution; commit Cargo.lock with your application.

### Other useful commands

```sh
# Build only (no board required).
cargo build --release --locked --bins --examples
cargo fmt --all -- --check
cargo clippy --locked --bins --examples -- -D warnings

# Configure EYESPI pins in a deselected idle state, then blink LD1.
cargo eyespi
# Equivalent: cargo run --release --locked --example eyespi

# Flash an already-built ELF without entering the RTT monitor, then reset it.
probe-rs download --chip STM32U575ZITxQ --connect-under-reset \
  target/thumbv8m.main-none-eabihf/release/ltplayer
probe-rs reset --chip STM32U575ZITxQ --connect-under-reset

# More verbose firmware logs.
DEFMT_LOG=debug cargo flash-board
```

To select among multiple connected probes, run the ELF directly with `probe-rs run --probe <selector>` and the same chip/connect options, or add the selector to the Cargo runner. See `probe-rs list` / `probe-rs run --help`.

On Linux you may need probe-rs [udev rules](https://probe.rs/docs/getting-started/probe-setup/). On Windows check ST-LINK USB drivers; macOS normally needs no additional ST-LINK driver. If the board is absent, first check USB data capability, power, and the selected connector. Close other debugger applications before attaching. The runner already connects under reset; do not use mass erase or change protection bits as a routine retry.

### `No debug probes were found` on macOS

This is USB probe discovery, before the STM32 firmware, SPI wiring, or TrustZone is involved. Check the USB connection first:

1. Use the **ST-LINK USB connector**, not the MCU/user USB connector. Board power LEDs alone do not establish a USB data connection.
2. Try a known-good **data** cable, and connect directly to the Mac where possible (bypass hubs/docks; try another adapter/port).
3. Unlock the Mac, reconnect, and accept **Allow accessory to connect** if prompted. Check System Settings → Privacy & Security → Allow accessories to connect if available; you should not need to disable security globally.
4. Check macOS USB enumeration:

   ```sh
   ioreg -p IOUSB -w 0
   probe-rs list
   ```

   Look for an ST-LINK/STMicroelectronics USB device. If it is absent, solve the cable/connector/accessory-approval issue first. If it is present but probe-rs still lists nothing, capture its device name and USB vendor/product IDs for further diagnosis.

Do not change BOOT/TrustZone/RDP settings or mass-erase the target to fix a missing USB probe. Use `cargo flash-board` after discovery works. `cargo flash` is the separately installed cargo-flash command, not this project's alias.

## EYESPI wiring

**Disconnect power before wiring. Use the Nucleo's 3V3 supply and 3.3 V logic, with a shared ground. Do not connect 5 V to signal pins.** Check the eventual display's voltage/current requirements and the Nucleo regulator budget before powering its backlight. The adapter itself does not provide level shifting, power regulation, or a display controller. If using a separate regulated display supply, share ground and avoid tying together competing power outputs.

Use the Arduino **D-labels on this exact board**, not connector pin numbers from a different Nucleo model. The following assignments match `examples/eyespi.rs`; the control-pin choices are arbitrary software GPIO, not hardware NSS.

| EYESPI label | Nucleo header label | STM32 pin | Starter behavior |
| --- | --- | --- | --- |
| Vin | 3V3 | power | Display power, subject to display requirements |
| Gnd | GND | ground | Common ground |
| SCK | D13 | PA5 / SPI1 | SPI clock, mode 0 |
| MOSI | D11 | PA7 / SPI1 | MCU to display |
| MISO | D12 | PA6 / SPI1 | Optional for write-only display; used for SD/touch reads |
| TCS | D10 | PD14 | Display chip select, held high (inactive) |
| DC | D9 | PD15 | Data/command, held low |
| RST | D8 | PF12 | Reset, held high (not asserted) |
| Lite | D5 | PE11 | Backlight control, held low (off) |
| TSCS | D7 | PF13 | Optional SPI touch select, held high |
| SDCS | D4 | PF14 | Optional microSD select, held high |
| MEMCS | D6 | PE9 | Optional external memory select, held high |

The example initializes SPI1 at a requested **1 MHz, mode 0, MSB first**, but intentionally sends **no SPI traffic or display reset sequence**. All configured objects remain in scope throughout the heartbeat loop. Backlight remains off until you add a real display driver; a blank screen is expected. Holding unused selects high prevents other peripherals on the display from driving the shared MISO line. For protection during MCU reset/boot (before firmware takes control), verify the display has pull-ups on active-low CS lines, or add appropriate external pull-ups to 3V3.

`SCL`, `SDA`, `INT`, `BUSY`, `GP1`, and `GP2` are **not configured or wired by this starter**. Capacitive touch often uses I2C (`SCL`/`SDA`) plus optional `INT`, while resistive touch may use SPI with `TSCS`. E-paper needs a controller-specific driver and may need `BUSY`. Determine these from the actual display. Never assume the breakout pinout specifies a touch controller.

Use the correct 18-pin, 0.5 mm EYESPI FPC cable and follow [Adafruit's cable insertion instructions](https://learn.adafruit.com/adafruit-eyespi-breakout-board/plugging-in-an-eyespi-cable). Check orientation against both connector guides; do not force the latch.

**LD1 routing:** the starter expects factory routing **SB21 ON / SB23 OFF**, which puts green LD1 on PC7. The alternative routes LD1 to PA5, the SPI clock; keep LD1 on PC7 when using this wiring.

## Add your eventual display

Start from `examples/eyespi.rs`. Once you have a product link, determine:

- Display controller (for example ST7789, ILI9341, or GC9A01), panel dimensions, offsets, color order, and orientation.
- SPI mode and maximum clock rate; begin slowly with short wires.
- Reset/power-up delays and backlight behavior specified by the display.
- Touch controller and bus, if present, separately from the display controller.

The blocking Embassy SPI implements `embedded-hal` 1.0 `SpiBus<u8>`. A driver that expects `SpiDevice` needs a CS-owning adapter (such as `embedded-hal-bus::spi::ExclusiveDevice`) around the bus and display CS; do not pass a raw bus where a device is required. Drivers such as `mipidsi` can then use a compatible SPI display interface and `embedded-graphics` for drawing. Add only the driver that matches your panel. If you later share SPI with SD/touch/memory, use a proper shared-bus manager and one independent CS per device, switching bus settings where needed. Do not construct multiple owners of SPI1.

No panel init sequence, drawing code, touch driver, filesystem, or SD driver is included yet. The starter stays usable without external peripherals and makes no assumptions that could send invalid commands to an unknown panel.

## Layout and memory

- [src/main.rs](src/main.rs): standalone LED/RTT bring-up firmware; safe to run with no display connected.
- [examples/eyespi.rs](examples/eyespi.rs): SPI/control-pin scaffolding; no controller-specific commands.
- [.cargo/config.toml](.cargo/config.toml): Cortex-M33 target, probe-rs runner, linker flags, and aliases.
- [Cargo.toml](Cargo.toml) / [Cargo.lock](Cargo.lock): dependencies and locked versions.
- [.github/workflows/ci.yml](.github/workflows/ci.yml): formatting, release build, and Clippy on GitHub Actions.

Embassy's `stm32u575zi` chip feature and `memory-x` generate the chip memory layout. There is no hand-copied memory map, bootloader offset, heap, or TrustZone partition. The project uses the internal default clock configuration and an Embassy hardware timer for async delays. Debug information is retained in release builds for RTT decoding and debugging. Build/Clippy checks do not prove that physical wiring or option bytes are correct; real board validation is still required.

## Hardware references

- [NUCLEO-U575ZI-Q product page](https://www.st.com/en/evaluation-tools/nucleo-u575zi-q.html) (ST board manual/schematics are under Documentation).
- [ST's exact-board configuration and MCU identity](https://github.com/STMicroelectronics/STM32_open_pin_data/blob/master/boards/B54_Nucleo_NUCLEO-U575ZI-Q_STM32U575ZI_Board_AllConfig.ioc).
- [ST's LED BSP definitions](https://github.com/STMicroelectronics/stm32u5xx-nucleo-bsp/blob/main/stm32u5xx_nucleo.h).
- [Exact-board D-pin mapping](https://github.com/stm32duino/Arduino_Core_STM32/blob/main/variants/STM32U5xx/U575Z%28G-I%29TxQ_U585ZITxQ/variant_NUCLEO_U575ZI_Q.cpp) and [LD1 solder-bridge routing](https://github.com/stm32duino/Arduino_Core_STM32/blob/main/variants/STM32U5xx/U575Z%28G-I%29TxQ_U585ZITxQ/variant_NUCLEO_U575ZI_Q.h).
- [ST's TrustZone-disabled template requirements](https://github.com/STMicroelectronics/STM32CubeU5/blob/main/Projects/NUCLEO-U575ZI-Q/Templates/TrustZoneDisabled/readme.html).
- [Adafruit EYESPI breakout pinouts](https://learn.adafruit.com/adafruit-eyespi-breakout-board/pinouts).
- [Embassy STM32 documentation](https://docs.embassy.dev/embassy-stm32/).
- [probe-rs documentation](https://probe.rs/docs/getting-started/).
