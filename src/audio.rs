use audio::AudioOutput;
use embassy_time::Timer;
use exfat_slim::asynchronous::file::OpenOptions;
use mbop3::{Decoder, MAX_SAMPLES_PER_FRAME};

use crate::{
  file_manager::FS,
  state::{State, STATE},
};

pub async fn play_file(
  fs: &mut FS<'static, 'static>,
  track_idx: usize,
  track_name: &str,
  output: &mut AudioOutput,
) -> Result<(), ()> {
  let options = OpenOptions::new().read(true);
  let mut file = fs.open(track_name, options).await.map_err(|_| ())?;

  let mut mp3_buf = [0u8; 4096];
  let mut pcm_samples = [0i16; MAX_SAMPLES_PER_FRAME];
  let mut decoder = Decoder::new();
  let mut total_samples: u64 = 0;
  let mut last_elapsed_sec: u32 = 0;

  loop {
    {
      let state = STATE.lock().await;
      match &*state {
        State::Playing(playback) => {
          if playback.index != track_idx {
            return Ok(());
          }
          if !playback.is_playing {
            drop(state);
            Timer::after_millis(50).await;
            continue;
          }
        }
        _ => return Ok(()),
      }
    }

    let bytes_read = match file.read(fs, &mut mp3_buf).await {
      Ok(Some(n)) if n > 0 => n,
      _ => {
        let mut state = STATE.lock().await;
        if let State::Playing(playback) = &mut *state {
          if playback.index == track_idx {
            *state = State::Menu(track_idx);
          }
        }
        return Ok(());
      }
    };

    let mut offset = 0;
    while offset < bytes_read {
      {
        let state = STATE.lock().await;
        match &*state {
          State::Playing(playback) => {
            if playback.index != track_idx || !playback.is_playing {
              break;
            }
          }
          _ => return Ok(()),
        }
      }

      let (samples, info) =
        decoder.decode_frame(&mp3_buf[offset..bytes_read], Some(&mut pcm_samples));

      if info.frame_bytes > 0 {
        offset += info.frame_bytes as usize;
        let total_channel_samples = samples * info.channels as usize;
        let audio = &pcm_samples[..total_channel_samples];

        output.write(audio).await.map_err(|_| ())?;

        total_samples += samples as u64;

        if info.hz > 0 {
          let current_elapsed = (total_samples / info.hz as u64) as u32;
          if current_elapsed != last_elapsed_sec {
            last_elapsed_sec = current_elapsed;
            let mut state = STATE.lock().await;
            if let State::Playing(playback) = &mut *state {
              if playback.index == track_idx {
                playback.elapsed = current_elapsed;
              }
            }
          }

          let frame_millis = (samples as u64 * 1000) / info.hz as u64;
          if frame_millis > 0 {
            Timer::after_millis(frame_millis).await;
          }
        }
      } else {
        break;
      }
    }
  }
}
