#![no_std]

use embassy_stm32::sai;
use embedded_hal::i2c::I2c;

pub mod cs43131;
pub use cs43131::{ConnectionType, Cs43131, Error as Cs43131Error, OutputMode, DEFAULT_I2C_ADDR};

pub struct AudioOutput {
  volume: u8,
}

impl AudioOutput {
  pub fn new() -> Self {
    Self { volume: 70 }
  }

  pub async fn write(&mut self, _buf: &[i16]) -> Result<(), ()> {
    Ok(())
  }

  pub fn set_volume(&mut self, volume: u8) {
    self.volume = volume;
  }
}

impl Default for AudioOutput {
  fn default() -> Self {
    Self::new()
  }
}

pub struct HardwareAudioOutput<'a, I2C: I2c, T: sai::Instance> {
  sai: sai::Sai<'a, T, u16>,
  cs43131: cs43131::Cs43131<I2C>,
}

impl<'a, I2C: I2c, T: sai::Instance> HardwareAudioOutput<'a, I2C, T> {
  pub fn new(i2c: I2C, i2c_addr: u8, sai: sai::Sai<'a, T, u16>) -> Self {
    let cs43131 = cs43131::Cs43131::new(i2c, i2c_addr);

    Self { sai, cs43131 }
  }

  pub fn set_volume(&mut self, volume: u8) -> Result<(), cs43131::Error<I2C::Error>> {
    self.cs43131.set_volume(volume)
  }

  pub fn set_output_mode(
    &mut self,
    mode: cs43131::OutputMode,
  ) -> Result<(), cs43131::Error<I2C::Error>> {
    self.cs43131.set_output_mode(mode)
  }

  pub async fn write(&mut self, buf: &[i16]) -> Result<(), sai::Error> {
    let u16_buf = unsafe { core::slice::from_raw_parts(buf.as_ptr() as *const u16, buf.len()) };

    self.sai.write(u16_buf).await
  }
}
