use embedded_hal::i2c::I2c;

pub const DEFAULT_I2C_ADDR: u8 = 0x30;

pub enum Error<E> {
  I2c(E),
  DeviceNotFound,
}

impl<E> From<E> for Error<E> {
  fn from(e: E) -> Self {
    Self::I2c(e)
  }
}

pub enum ConnectionType {
  MM35,
  MM44,
}

pub enum OutputMode {
  Disconnected,
  Connected(ConnectionType),
}

pub struct Cs43131<I2C: I2c> {
  i2c: I2C,
  address: u8,
}

impl<I2C: I2c> Cs43131<I2C> {
  pub fn new(i2c: I2C, address: u8) -> Self {
    Self { i2c, address }
  }

  pub fn read_register(&mut self, reg: u32) -> Result<u8, I2C::Error> {
    let bytes = [
      ((reg >> 16) & 0xFF) as u8,
      ((reg >> 8) & 0xFF) as u8,
      (reg & 0xFF) as u8,
    ];

    let mut read = [0u8];

    self.i2c.write_read(self.address, &bytes, &mut read)?;
    Ok(read[0])
  }

  pub fn write_register(&mut self, reg: u32, value: u8) -> Result<(), I2C::Error> {
    let bytes = [
      ((reg >> 16) & 0xFF) as u8,
      ((reg >> 8) & 0xFF) as u8,
      (reg & 0xFF) as u8,
      value,
    ];

    self.i2c.write(self.address, &bytes)
  }

  pub fn init(&mut self) -> Result<(), Error<I2C::Error>> {
    let (id_a, id_b) = (self.read_register(0x010000)?, self.read_register(0x010001)?);

    if (id_a, id_b) != (0x43, 0x13) {
      return Err(Error::DeviceNotFound); // idk what to  do here
    }

    self.write_register(0x020000, 0x00)?;

    // 44.1kHz sample rate
    self.write_register(0x01000B, 0x01)?;
    // 16-bit resolution
    self.write_register(0x01000C, 0x02)?;
    // standard i2s timing
    self.write_register(0x040019, 0x0A)?;
    // channels 1 and 2 on
    self.write_register(0x05000A, 0x0E)?;
    self.write_register(0x05000B, 0x0E)?;

    self.write_register(0x080000, 0x30)?;

    self.set_volume(70)?;

    Ok(())
  }

  pub fn set_volume(&mut self, volume: u8) -> Result<(), Error<I2C::Error>> {
    let volume = volume.min(100);

    let value = if volume == 0 {
      0xFF
    } else {
      ((100 - volume) as u16 * 96 / 100) as u8
    };

    self.write_register(0x090001, value)?;
    self.write_register(0x090002, value)?;

    Ok(())
  }

  pub fn set_output_mode(&mut self, mode: OutputMode) -> Result<(), Error<I2C::Error>> {
    match mode {
      OutputMode::Disconnected => {
        self.write_register(0x080000, 0x00)?;
      }
      OutputMode::Connected(ConnectionType::MM35) => {
        self.write_register(0x080000, 0x30)?;
      }
      OutputMode::Connected(ConnectionType::MM44) => {
        self.write_register(0x080000, 0x33)?;
      }
    }

    Ok(())
  }
}
