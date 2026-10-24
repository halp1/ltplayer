use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics_simulator::{
  sdl2::Keycode, OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};

#[path = "../../src/renderer.rs"]
mod renderer;
use renderer::{CurrentScreen, Renderer};

fn main() -> Result<(), core::convert::Infallible> {
  let display: SimulatorDisplay<Rgb565> = SimulatorDisplay::new(Size::new(240, 320));
  let output_settings = OutputSettingsBuilder::new().scale(2).build();
  let mut window = Window::new("ltplayer UI Simulator (240x320)", &output_settings);

  let sample_files = [
    "Bohemian Rhapsody.mp3",
    "Hotel California.mp3",
    "Stairway to Heaven.mp3",
    "Comfortably Numb.mp3",
    "Sweet Child O' Mine.mp3",
    "Billie Jean.mp3",
    "Imagine.mp3",
    "Like a Rolling Stone.mp3",
    "Smells Like Teen Spirit.mp3",
    "Sultans of Swing.mp3",
  ];

  let mut selected_idx = 0;
  let mut current_screen = CurrentScreen::FileList;
  let mut is_playing = true;
  let mut elapsed_secs = 83; // 1:23
  let total_secs = 354; // 5:54

  let mut renderer = Renderer::new(display);

  'running: loop {
    match current_screen {
      CurrentScreen::FileList => {
        renderer.render_file_list(&sample_files, selected_idx)?;
      }
      CurrentScreen::NowPlaying => {
        renderer.render_player(
          sample_files[selected_idx],
          elapsed_secs,
          total_secs,
          is_playing,
        )?;
      }
    }

    window.update(renderer.display());

    for event in window.events() {
      match event {
        SimulatorEvent::Quit => break 'running,
        SimulatorEvent::KeyDown { keycode, .. } => match (current_screen, keycode) {
          // File List Navigation
          (CurrentScreen::FileList, Keycode::Up) => {
            if selected_idx > 0 {
              selected_idx -= 1;
            } else {
              selected_idx = sample_files.len() - 1;
            }
          }
          (CurrentScreen::FileList, Keycode::Down) => {
            selected_idx = (selected_idx + 1) % sample_files.len();
          }
          (CurrentScreen::FileList, Keycode::Return) => {
            current_screen = CurrentScreen::NowPlaying;
            elapsed_secs = 0;
            is_playing = true;
          }

          // Player Controls
          (CurrentScreen::NowPlaying, Keycode::Space) => {
            is_playing = !is_playing;
          }
          (CurrentScreen::NowPlaying, Keycode::Right) => {
            elapsed_secs = (elapsed_secs + 10).min(total_secs);
          }
          (CurrentScreen::NowPlaying, Keycode::Left) => {
            elapsed_secs = elapsed_secs.saturating_sub(10);
          }
          (CurrentScreen::NowPlaying, Keycode::Escape)
          | (CurrentScreen::NowPlaying, Keycode::Backspace) => {
            current_screen = CurrentScreen::FileList;
          }
          _ => {}
        },
        _ => {}
      }
    }

    std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 fps
  }

  Ok(())
}
