use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

#[derive(Debug, Clone, Copy)]
pub struct PlaybackState {
  pub index: usize,
  pub elapsed: u32,
  pub total: u32,
  pub is_playing: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum State {
  Menu(usize),
  Playing(PlaybackState),
}

pub type StateMutex = Mutex<CriticalSectionRawMutex, State>;

pub static STATE: StateMutex = Mutex::new(State::Menu(0));
