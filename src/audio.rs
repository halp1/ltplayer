use audio::AudioOutput;
use embassy_stm32::sai;
use embedded_hal::i2c::I2c;
use exfat_slim::asynchronous::file::File;
use mbop3::Decoder;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

use crate::file_manager::FS;

#[derive(Clone, Copy, Debug)]
pub enum Command {
  Play,
  Pause,
  Next,
  Prev,
  Stop,
}

pub static COMMANDS: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();

pub enum PlayResult {
  Finished,
  Next,
  Prev,
  Stopped,
}

pub async fn play_file<'a, 'b, I2C: I2c, T: sai::Instance>(
  file: &mut File,
  fs: &mut FS<'a, 'b>,
  output: &mut AudioOutput<'_, I2C, T>,
) -> Result<PlayResult, ()> {
  let mut mp3_buf = [0u8; 4096];
  let mut pcm_samples = [0i16; 2304];

  let mut decoder = Decoder::new();

  loop {
    if let Ok(cmd) = COMMANDS.try_receive() {
      match cmd {
        Command::Pause => {
          defmt::info!("pause");

          loop {
            match COMMANDS.receive().await {
              Command::Play => {
                defmt::info!("unpause");
                break;
              }
              Command::Next => {
                return Ok(PlayResult::Next);
              }
              Command::Prev => {
                return Ok(PlayResult::Prev);
              }
              Command::Stop => {
                return Ok(PlayResult::Stopped);
              }
              _ => {}
            }
          }
        }
        Command::Next => return Ok(PlayResult::Next),
        Command::Prev => return Ok(PlayResult::Prev),
        Command::Stop => return Ok(PlayResult::Stopped),

        _ => {}
      }
    }

    let bytes_read = file.read(fs, &mut mp3_buf).await.map_err(|_| ())?;

    if let Some(bytes_read) = bytes_read {
      if bytes_read == 0 {
        defmt::info!("EOF");
        break;
      }

      let mut offset = 0;

      while offset < bytes_read {
        let (samples, info) =
          decoder.decode_frame(&mp3_buf[offset..bytes_read], Some(&mut pcm_samples));

        if info.frame_bytes > 0 {
          offset += info.frame_bytes as usize;

          let samples = samples * info.channels as usize;
          let audio = &pcm_samples[..samples];

          output.write(audio).await.map_err(|_| ())?;
        } else {
          break;
        }
      }
    }
  }

  Ok(PlayResult::Finished)
}
