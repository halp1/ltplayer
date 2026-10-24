#![no_std]

use embassy_time::Delay;
use embedded_hal::{digital::OutputPin, spi::SpiBus};
use embedded_hal_bus::spi::ExclusiveDevice;
use mipidsi::{interface::SpiInterface, models::ST7789};

pub use embedded_graphics;

pub type Display<'a, SPI, CS, DC, RST> =
  mipidsi::Display<SpiInterface<'a, ExclusiveDevice<SPI, CS, Delay>, DC>, ST7789, RST>;

pub fn new<'a, SPI, CS, DC, RST>(
  spi: SPI,
  cs: CS,
  dc: DC,
  rst: RST,
  buffer: &'a mut [u8],
) -> Result<Display<'a, SPI, CS, DC, RST>, ()>
where
  SPI: SpiBus,
  CS: OutputPin,
  DC: OutputPin,
  RST: OutputPin,
{
  let spi_device = ExclusiveDevice::new(spi, cs, Delay).map_err(|_| ())?;

  let di = SpiInterface::new(spi_device, dc, buffer);

  let display = mipidsi::Builder::new(ST7789, di)
    .reset_pin(rst)
    .display_size(240, 320)
    .invert_colors(mipidsi::options::ColorInversion::Inverted)
    .init(&mut Delay)
    .map_err(|_| ())?;

  Ok(display)
}
