#![no_std]

use embassy_stm32::sai;
use embassy_time::Timer;
use embedded_hal::{digital::OutputPin, i2c::I2c};

pub mod cs43131;
pub mod jack;
pub use cs43131::{ConnectionType, Cs43131, DEFAULT_I2C_ADDR, Error as Cs43131Error, OutputMode};
pub use jack::{JackRouter, JackSelector};

/// Time for the DAC soft mute to settle before the analog switches move.
const MUTE_SETTLE_MS: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputError {
  Dac,
  Switch,
}

pub struct AudioOutput<SEL: OutputPin> {
  volume: u8,
  router: JackRouter<SEL>,
  mode: OutputMode,
}

impl<SEL: OutputPin> AudioOutput<SEL> {
  pub fn new(sel: SEL) -> Self {
    Self {
      volume: 70,
      router: JackRouter::new(sel),
      mode: OutputMode::Disconnected,
    }
  }

  pub async fn write(&mut self, _buf: &[i16]) -> Result<(), ()> {
    Ok(())
  }

  pub fn set_volume(&mut self, volume: u8) {
    self.volume = volume;
  }

  pub async fn apply_output(&mut self, mode: OutputMode) -> Result<(), OutputError> {
    if let OutputMode::Connected(connection) = mode {
      self
        .router
        .route(connection)
        .map_err(|_| OutputError::Switch)?;
    }
    self.mode = mode;
    Ok(())
  }

  pub fn output_mode(&self) -> OutputMode {
    self.mode
  }
}

pub struct HardwareAudioOutput<'a, I2C: I2c, T: sai::Instance, SEL: OutputPin> {
  sai: sai::Sai<'a, T, u16>,
  cs43131: cs43131::Cs43131<I2C>,
  router: JackRouter<SEL>,
  mode: OutputMode,
}

impl<'a, I2C: I2c, T: sai::Instance, SEL: OutputPin> HardwareAudioOutput<'a, I2C, T, SEL> {
  pub fn new(i2c: I2C, i2c_addr: u8, sai: sai::Sai<'a, T, u16>, sel: SEL) -> Self {
    let cs43131 = cs43131::Cs43131::new(i2c, i2c_addr);

    Self {
      sai,
      cs43131,
      router: JackRouter::new(sel),
      mode: OutputMode::Disconnected,
    }
  }

  pub fn set_volume(&mut self, volume: u8) -> Result<(), cs43131::Error<I2C::Error>> {
    self.cs43131.set_volume(volume)
  }

  /// Mute, move the analog switches, reconfigure the headphone amp, then
  /// unmute only if a jack is connected. Ordered to avoid pops.
  pub async fn apply_output(&mut self, mode: OutputMode) -> Result<(), OutputError> {
    self.cs43131.set_mute(true).map_err(|_| OutputError::Dac)?;
    Timer::after_millis(MUTE_SETTLE_MS).await;

    if let OutputMode::Connected(connection) = mode {
      self
        .router
        .route(connection)
        .map_err(|_| OutputError::Switch)?;
    }
    self
      .cs43131
      .set_output_mode(mode)
      .map_err(|_| OutputError::Dac)?;
    self.mode = mode;

    if matches!(mode, OutputMode::Connected(_)) {
      self.cs43131.set_mute(false).map_err(|_| OutputError::Dac)?;
    }

    Ok(())
  }

  pub fn output_mode(&self) -> OutputMode {
    self.mode
  }

  pub async fn write(&mut self, buf: &[i16]) -> Result<(), sai::Error> {
    let u16_buf = unsafe { core::slice::from_raw_parts(buf.as_ptr() as *const u16, buf.len()) };

    self.sai.write(u16_buf).await
  }
}
