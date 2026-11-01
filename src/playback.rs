use embassy_stm32::gpio::Output;
use embassy_time::Timer;

use crate::{
  audio::{Dac, play_file, sync_output},
  file_manager::{FS, FileList},
  jack::OUTPUT,
  state::{STATE, State},
};

#[embassy_executor::task]
pub async fn playback_task(
  fs: &'static mut FS<'static, 'static>,
  file_list: FileList,
  jack_sel: Output<'static>,
) {
  let mut output = Dac::new(jack_sel);
  let mut jack = OUTPUT.receiver().expect("OUTPUT receiver slots exhausted");

  loop {
    sync_output(&mut output, &mut jack).await;

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
        let _ = play_file(fs, track_idx, track_name, &mut output, &mut jack).await;
      }
    }

    Timer::after_millis(50).await;
  }
}
