use embassy_futures::select::select4;
use embassy_stm32::gpio::Input;
use embassy_time::Timer;

use crate::{
  file_manager::FileList,
  state::{PlaybackState, STATE, State},
};

struct Button<'a> {
  input: Input<'a>,
}

impl<'a> Button<'a> {
  pub fn new(input: Input<'a>) -> Self {
    Self { input }
  }

  pub async fn wait(&self) {
    while self.input.is_high() {
      Timer::after_millis(10).await;
    }

    Timer::after_millis(30).await;

    while self.input.is_low() {
      Timer::after_millis(10).await;
    }

    Timer::after_millis(30).await;
  }
}

#[embassy_executor::task]
pub async fn controls_task(
  file_list: FileList,
  prev: Input<'static>,
  next: Input<'static>,
  select: Input<'static>,
  back: Input<'static>,
) -> ! {
  let (prev, next, select, back) = (
    Button::new(prev),
    Button::new(next),
    Button::new(select),
    Button::new(back),
  );

  loop {
    select4(
      async {
        prev.wait().await;

        let mut state = STATE.lock().await;

        match &mut *state {
          State::Menu(idx) => {
            if *idx > 0 {
              *idx -= 1;
            } else {
              *idx = file_list.len() - 1;
            }
          }
          State::Playing(state) => {
            if state.index > 0 {
              state.index -= 1
            } else {
              state.index = file_list.len() - 1;
            }

            state.is_playing = true;
          }
        }
      },
      async {
        next.wait().await;

        let mut state = STATE.lock().await;

        match &mut *state {
          State::Menu(idx) => {
            if *idx < file_list.len() - 1 {
              *idx += 1;
            } else {
              *idx = 0;
            }
          }
          State::Playing(state) => {
            if state.index < file_list.len() - 1 {
              state.index += 1
            } else {
              state.index = 0;
            }

            state.is_playing = true;
          }
        }
      },
      async {
        select.wait().await;

        let mut state = STATE.lock().await;

        match &mut *state {
          State::Menu(idx) => {
            *state = State::Playing(PlaybackState {
              index: *idx,
              elapsed: 0,
              total: 0,
              is_playing: true,
            });
          }
          State::Playing(p) => {
            p.is_playing = !p.is_playing;
          }
        }
      },
      async {
        back.wait().await;

        let mut state = STATE.lock().await;

        if let State::Playing(PlaybackState { index, .. }) = *state {
          *state = State::Menu(index);
        }
      },
    )
    .await;
  }
}
