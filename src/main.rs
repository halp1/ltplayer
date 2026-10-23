#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::peripherals::SDMMC1;
use embassy_stm32::sdmmc::InterruptHandler;
use embassy_stm32::{
  gpio::{Level, Output, Speed},
  spi,
  time::Hertz,
};
use embassy_time::Timer;
use embedded_hal::spi::SpiBus;

pub mod file_manager;
pub mod audio;
pub mod renderer;

use panic_probe as _;

bind_interrupts!(struct Irqs {
  SDMMC1 => InterruptHandler<SDMMC1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
  let p = embassy_stm32::init(Default::default());

  let mut sdmmc = embassy_stm32::sdmmc::Sdmmc::new_4bit(
    p.SDMMC1,
    Irqs,
    p.PC12, // CK
    p.PD2,  // CMD
    p.PC8,  // D0
    p.PC9,  // D1
    p.PC10, // D2
    p.PC11, // D3
    Default::default(),
  );

  let mut sd = microsd::mount(&mut sdmmc, microsd::SdCardConfig::default())
    .await
    .expect("SD card mount failed");

  defmt::info!("sd up");

  let cs = Output::new(p.PD14, Level::High, Speed::High);
  let dc = Output::new(p.PD15, Level::Low, Speed::High);
  let rst = Output::new(p.PF12, Level::High, Speed::High);
  let _backlight = Output::new(p.PE11, Level::High, Speed::Low);

  // 2. SPI1 at 25 MHz (Blocking SPI):
  let mut display_spi_config = spi::Config::default();
  display_spi_config.frequency = Hertz::mhz(25);
  let spi = spi::Spi::new_blocking(p.SPI1, p.PA5, p.PA7, p.PA6, display_spi_config);

  // 3. Initialize the display:
  let mut display_buffer = [0_u8; 512];
  let mut display =
    display::new(spi, cs, dc, rst, &mut display_buffer).expect("Display init failed");

  defmt::info!("display up");
  loop {}
}
