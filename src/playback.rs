use audio::AudioOutput;
use embassy_time::Timer;

use crate::{
  audio::play_file,
  file_manager::{FileList, FS},
  state::{State, STATE},
};

#[embassy_executor::task]
pub async fn playback_task(fs: &'static mut FS<'static, 'static>, file_list: FileList) {
  let mut output = AudioOutput::new();

  loop {
    let current_target = {
      let state = STATE.lock().await;
      match &*state {
        State::Playing(playback) => Some(playback.index),
        _ => None,
      }
    };

    if let Some(track_idx) = current_target {
      if track_idx < file_list.len() {
        let track_name = file_list[track_idx].as_str();
        let _ = play_file(fs, track_idx, track_name, &mut output).await;
      }
    }

    Timer::after_millis(50).await;
  }
}
