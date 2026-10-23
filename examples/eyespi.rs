#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::spi::{Config, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::Timer;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
  let p = embassy_stm32::init(Default::default());
  let mut led = Output::new(p.PC7, Level::Low, Speed::Low);

  // Drive active-low selects inactive BEFORE configuring the shared bus.
  let _display_cs = Output::new(p.PD14, Level::High, Speed::Low); // D10 -> TCS
  let _touch_cs = Output::new(p.PF13, Level::High, Speed::Low); // D7 -> TSCS
  let _sd_cs = Output::new(p.PF14, Level::High, Speed::Low); // D4 -> SDCS
  let _memory_cs = Output::new(p.PE9, Level::High, Speed::Low); // D6 -> MEMCS
  let _dc = Output::new(p.PD15, Level::Low, Speed::Low); // D9 -> DC
  let _reset = Output::new(p.PF12, Level::High, Speed::Low); // D8 -> RST
  let _backlight = Output::new(p.PE11, Level::Low, Speed::Low); // D5 -> Lite (off)

  let mut config = Config::default();
  config.frequency = Hertz::mhz(1);
  // Default: mode 0, MSB first. Check these against your eventual display.
  let _spi = Spi::new_blocking(p.SPI1, p.PA5, p.PA7, p.PA6, config);
  //                                D13 SCK  D11 MOSI D12 MISO

  // Intentionally NO display commands, reset pulses, or SPI traffic here.
  // The breakout has no controller to identify. Add the actual display driver
  // and transfer ownership of _spi/_display_cs/_dc/_reset to it once selected.
  // Keep these locals in scope so the pins/bus remain configured.
  defmt::info!("EYESPI idle: SPI1 configured, all CS high, backlight off");
  defmt::info!("No display/touch/SD driver selected; no SPI traffic generated");

  loop {
    led.toggle();
    Timer::after_millis(500).await;
  }
}
