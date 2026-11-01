use audio::{JackSelector, OutputMode};
use embassy_stm32::gpio::Input;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::{Receiver, Watch};
use embassy_time::Timer;

use crate::state::{STATE, State};

/// Receivers: the playback task.
pub const OUTPUT_RECEIVERS: usize = 1;

pub static OUTPUT: Watch<CriticalSectionRawMutex, OutputMode, OUTPUT_RECEIVERS> = Watch::new();

pub type OutputReceiver = Receiver<'static, CriticalSectionRawMutex, OutputMode, OUTPUT_RECEIVERS>;

const POLL_MS: u64 = 10;
/// Consecutive matching samples before a detect pin change is accepted.
const STABLE_SAMPLES: u8 = 5;

/// A detect switch input, active low with a pull-up.
struct Detect<'a> {
  input: Input<'a>,
  stable: bool,
  count: u8,
}

impl<'a> Detect<'a> {
  fn new(input: Input<'a>) -> Self {
    let stable = input.is_low();
    Self {
      input,
      stable,
      count: 0,
    }
  }

  /// Returns the debounced plugged state.
  fn sample(&mut self) -> bool {
    let raw = self.input.is_low();
    if raw == self.stable {
      self.count = 0;
    } else {
      self.count += 1;
      if self.count >= STABLE_SAMPLES {
        self.stable = raw;
        self.count = 0;
      }
    }
    self.stable
  }
}

#[embassy_executor::task]
pub async fn jack_task(detect_35: Input<'static>, detect_44: Input<'static>) -> ! {
  let (mut detect_35, mut detect_44) = (Detect::new(detect_35), Detect::new(detect_44));
  let mut selector = JackSelector::new();
  let sender = OUTPUT.sender();

  let mut current = selector.update(detect_35.stable, detect_44.stable);
  sender.send(current);

  loop {
    Timer::after_millis(POLL_MS).await;

    let mode = selector.update(detect_35.sample(), detect_44.sample());
    if mode == current {
      continue;
    }
    current = mode;

    if mode == OutputMode::Disconnected {
      let mut state = STATE.lock().await;
      if let State::Playing(playback) = &mut *state {
        playback.is_playing = false;
      }
    }

    sender.send(mode);
  }
}
