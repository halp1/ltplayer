#![no_std]

// Ready for you to write!

use block_device_driver::BlockDevice;
use embassy_stm32::{
  sdmmc::{
    self,
    sd::{Card, CmdBlock, StorageDevice},
  },
  time::Hertz,
};
use exfat_slim::asynchronous::file_system::FileSystem;

#[derive(Debug, Clone, Copy)]
pub struct SdCardConfig {
  pub freq: Hertz,
}

impl Default for SdCardConfig {
  fn default() -> Self {
    Self {
      freq: Hertz(25_000_000),
    }
  }
}

pub struct SdCard<'a, 'b> {
  device: StorageDevice<'a, 'b, Card>,
  partition_offset: u32,
}

impl<'a, 'b> SdCard<'a, 'b> {
  pub async fn new(
    sdmmc: &'a mut sdmmc::Sdmmc<'b>,
    config: SdCardConfig,
  ) -> Result<SdCard<'a, 'b>, sdmmc::Error> {
    let mut cmd_block = CmdBlock::new();

    let mut device = StorageDevice::new_sd_card(sdmmc, &mut cmd_block, config.freq).await?;

    let mut sector_0 = [aligned::Aligned([0u8; 512])];

    device.read(0, &mut sector_0).await?;

    let partition_offset = if sector_0[0][3..11] == *b"EXFAT   " {
      0
    } else {
      if sector_0[0][510..512] == [0x55, 0xAA] {
        u32::from_le_bytes(sector_0[0][454..458].try_into().unwrap())
      } else {
        return Err(sdmmc::Error::UnsupportedCardType);
      }
    };

    Ok(SdCard {
      device,
      partition_offset,
    })
  }
}

impl<'a, 'b> BlockDevice<512> for SdCard<'a, 'b> {
  type Align = aligned::A4;
  type Error = sdmmc::Error;

  async fn read(
    &mut self,
    block_address: u32,
    data: &mut [aligned::Aligned<Self::Align, [u8; 512]>],
  ) -> Result<(), Self::Error> {
    self
      .device
      .read(block_address + self.partition_offset, data)
      .await
  }

  async fn write(
    &mut self,
    block_address: u32,
    data: &[aligned::Aligned<Self::Align, [u8; 512]>],
  ) -> Result<(), Self::Error> {
    self
      .device
      .write(block_address + self.partition_offset, data)
      .await
  }

  async fn size(&mut self) -> Result<u64, Self::Error> {
    self.device.size().await
  }
}

pub async fn mount<'a, 'b>(
  sdmmc: &'a mut sdmmc::Sdmmc<'b>,
  config: SdCardConfig,
) -> Result<FileSystem<SdCard<'a, 'b>, 512, 4>, sdmmc::Error> {
  let sd_card = SdCard::new(sdmmc, config).await?;
  Ok(exfat_slim::asynchronous::file_system::FileSystem::new(
    sd_card,
  ))
}
