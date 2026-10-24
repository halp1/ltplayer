use embedded_graphics::{
  mono_font::{
    MonoTextStyle,
    ascii::{FONT_6X10, FONT_7X13, FONT_9X15},
  },
  pixelcolor::Rgb565,
  prelude::*,
  primitives::{Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Triangle},
  text::{Alignment, Text},
};

#[allow(dead_code)]
pub enum UiState<'a> {
  Menu(usize),
  Playing {
    name: &'a str,
    elapsed: u32,
    total: u32,
    is_playing: bool,
  },
}

// -----------------------------------------------------------------------------
// Color Palette (Modern Dark / Audiophile Theme)
// -----------------------------------------------------------------------------
pub const COLOR_BG: Rgb565 = Rgb565::new(1, 2, 3); // Deep charcoal black
pub const COLOR_SURFACE: Rgb565 = Rgb565::new(3, 6, 8); // Dark card surface
pub const COLOR_BORDER: Rgb565 = Rgb565::new(8, 16, 18); // Subtle slate border
pub const COLOR_ACCENT: Rgb565 = Rgb565::new(0, 55, 31); // Electric Cyan
pub const COLOR_TEXT_PRIMARY: Rgb565 = Rgb565::WHITE;
pub const COLOR_TEXT_MUTED: Rgb565 = Rgb565::new(20, 40, 20); // Silver / grey

// -----------------------------------------------------------------------------
// Screen States
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentScreen {
  FileList,
  NowPlaying,
}

pub struct Renderer<T>
where
  T: DrawTarget<Color = Rgb565>,
{
  display: T,
}

impl<T> Renderer<T>
where
  T: DrawTarget<Color = Rgb565>,
{
  pub fn new(display: T) -> Self {
    Self { display }
  }

  /// Borrow the underlying display.
  pub fn display(&mut self) -> &mut T {
    &mut self.display
  }

  pub fn clear(&mut self) -> Result<(), T::Error> {
    self.display.clear(COLOR_BG)
  }

  // ===========================================================================
  // SCREEN 1: File List (Playlist Browser)
  // ===========================================================================
  /// Render the scrollable file list screen.
  ///
  /// - Displays 7 items on screen at a time.
  /// - Automatically scrolls the view window so `selected_index` is always visible.
  /// - Highlights the active item with an accent bar and pointer arrow.
  pub fn render_file_list<S: AsRef<str>>(
    &mut self,
    files: &[S],
    selected_index: usize,
  ) -> Result<(), T::Error> {
    const VISIBLE_COUNT: usize = 12;
    const ITEM_HEIGHT: i32 = 25;
    const LIST_TOP: i32 = 10;

    if files.is_empty() {
      self.display.clear(COLOR_BG)?;
      let empty_style = MonoTextStyle::new(&FONT_7X13, COLOR_TEXT_MUTED);
      Text::with_alignment(
        "No MP3 files found",
        Point::new(120, 110),
        empty_style,
        Alignment::Center,
      )
      .draw(&mut self.display)?;
    } else {
      let window_start = if selected_index >= VISIBLE_COUNT {
        selected_index - VISIBLE_COUNT + 1
      } else {
        0
      };

      for slot in 0..VISIBLE_COUNT {
        let file_idx = window_start + slot;
        let y = LIST_TOP + (slot as i32 * ITEM_HEIGHT);

        if file_idx >= files.len() {
          Rectangle::new(Point::new(4, y), Size::new(232, 23))
            .into_styled(PrimitiveStyle::with_fill(COLOR_BG))
            .draw(&mut self.display)?;
          continue;
        }

        let is_selected = file_idx == selected_index;
        let (card_bg, card_border, text_color) = if is_selected {
          (COLOR_SURFACE, COLOR_ACCENT, COLOR_TEXT_PRIMARY)
        } else {
          (COLOR_BG, COLOR_BG, COLOR_TEXT_MUTED)
        };

        Rectangle::new(Point::new(4, y), Size::new(232, 23))
          .into_styled(
            PrimitiveStyleBuilder::new()
              .fill_color(card_bg)
              .stroke_color(card_border)
              .stroke_width(1)
              .build(),
          )
          .draw(&mut self.display)?;

        let indicator = if is_selected { ">" } else { " " };
        Text::new(
          indicator,
          Point::new(10, y + 16),
          MonoTextStyle::new(&FONT_7X13, COLOR_ACCENT),
        )
        .draw(&mut self.display)?;

        Text::new(
          files[file_idx].as_ref(),
          Point::new(24, y + 16),
          MonoTextStyle::new(&FONT_7X13, text_color),
        )
        .draw(&mut self.display)?;
      }
    }

    Ok(())
  }

  // ===========================================================================
  // SCREEN 2: Now Playing (Player & Artwork)
  // ===========================================================================
  /// Render the player screen with album art placeholder, track name, play/pause icon, and seekbar.
  pub fn render_player(
    &mut self,
    name: &str,
    elapsed: u32,
    total: u32,
    is_playing: bool,
  ) -> Result<(), T::Error> {
    const ART_X: i32 = 60;
    const ART_Y: i32 = 24;
    const ART_SIZE: u32 = 120;

    Rectangle::new(Point::new(ART_X, ART_Y), Size::new(ART_SIZE, ART_SIZE))
      .into_styled(
        PrimitiveStyleBuilder::new()
          .fill_color(COLOR_SURFACE)
          .stroke_color(COLOR_BORDER)
          .stroke_width(1)
          .build(),
      )
      .draw(&mut self.display)?;

    let note_head_style = PrimitiveStyle::with_fill(COLOR_ACCENT);
    let note_line_style = PrimitiveStyle::with_stroke(COLOR_ACCENT, 3);

    Circle::new(Point::new(ART_X + 30, ART_Y + 74), 20)
      .into_styled(note_head_style)
      .draw(&mut self.display)?;
    Line::new(
      Point::new(ART_X + 47, ART_Y + 84),
      Point::new(ART_X + 47, ART_Y + 36),
    )
    .into_styled(note_line_style)
    .draw(&mut self.display)?;

    Circle::new(Point::new(ART_X + 68, ART_Y + 62), 20)
      .into_styled(note_head_style)
      .draw(&mut self.display)?;
    Line::new(
      Point::new(ART_X + 85, ART_Y + 72),
      Point::new(ART_X + 85, ART_Y + 26),
    )
    .into_styled(note_line_style)
    .draw(&mut self.display)?;

    Line::new(
      Point::new(ART_X + 47, ART_Y + 36),
      Point::new(ART_X + 85, ART_Y + 26),
    )
    .into_styled(PrimitiveStyle::with_stroke(COLOR_ACCENT, 5))
    .draw(&mut self.display)?;

    let track_style = MonoTextStyle::new(&FONT_9X15, COLOR_TEXT_PRIMARY);
    Text::with_alignment(name, Point::new(120, 172), track_style, Alignment::Center)
      .draw(&mut self.display)?;

    self.update_progress(elapsed, total)?;
    self.update_play_pause(is_playing)?;

    Ok(())
  }

  pub fn update_progress(&mut self, elapsed: u32, total: u32) -> Result<(), T::Error> {
    const BAR_X: i32 = 20;
    const BAR_Y: i32 = 208;
    const BAR_WIDTH: u32 = 200;
    const BAR_HEIGHT: u32 = 6;

    Rectangle::new(Point::new(BAR_X, BAR_Y), Size::new(BAR_WIDTH, BAR_HEIGHT))
      .into_styled(PrimitiveStyle::with_fill(COLOR_SURFACE))
      .draw(&mut self.display)?;

    let fill_width = if total > 0 {
      ((elapsed.min(total) as u64 * BAR_WIDTH as u64) / total as u64) as u32
    } else {
      0
    };

    if fill_width > 0 {
      Rectangle::new(Point::new(BAR_X, BAR_Y), Size::new(fill_width, BAR_HEIGHT))
        .into_styled(PrimitiveStyle::with_fill(COLOR_ACCENT))
        .draw(&mut self.display)?;
    }

    Rectangle::new(Point::new(BAR_X, BAR_Y + 8), Size::new(50, 14))
      .into_styled(PrimitiveStyle::with_fill(COLOR_BG))
      .draw(&mut self.display)?;

    let time_style = MonoTextStyle::new(&FONT_6X10, COLOR_TEXT_MUTED);
    let mut time_buf_a = [0u8; 8];
    let elapsed_str = format_time(elapsed, &mut time_buf_a);
    Text::new(elapsed_str, Point::new(BAR_X, BAR_Y + 16), time_style).draw(&mut self.display)?;

    let mut time_buf_b = [0u8; 8];
    let total_str = format_time(total, &mut time_buf_b);
    Text::with_alignment(
      total_str,
      Point::new(BAR_X + BAR_WIDTH as i32, BAR_Y + 16),
      time_style,
      Alignment::Right,
    )
    .draw(&mut self.display)?;

    Ok(())
  }

  pub fn update_play_pause(&mut self, is_playing: bool) -> Result<(), T::Error> {
    Rectangle::new(Point::new(110, 250), Size::new(26, 35))
      .into_styled(PrimitiveStyle::with_fill(COLOR_BG))
      .draw(&mut self.display)?;

    if is_playing {
      Rectangle::new(Point::new(114, 258), Size::new(4, 20))
        .into_styled(PrimitiveStyle::with_fill(COLOR_TEXT_PRIMARY))
        .draw(&mut self.display)?;
      Rectangle::new(Point::new(122, 258), Size::new(4, 20))
        .into_styled(PrimitiveStyle::with_fill(COLOR_TEXT_PRIMARY))
        .draw(&mut self.display)?;
    } else {
      Triangle::new(
        Point::new(114, 257),
        Point::new(114, 279),
        Point::new(132, 268),
      )
      .into_styled(PrimitiveStyle::with_fill(COLOR_TEXT_PRIMARY))
      .draw(&mut self.display)?;
    }

    Ok(())
  }

  #[allow(dead_code)]
  pub fn render(&mut self, files: &[impl AsRef<str>], state: &UiState) -> Result<(), T::Error> {
    match state {
      UiState::Menu(idx) => self.render_file_list(files, *idx),
      UiState::Playing {
        name,
        elapsed,
        total,
        is_playing,
      } => self.render_player(name, *elapsed, *total, *is_playing),
    }
  }
}

// -----------------------------------------------------------------------------
// Pure no_std Formatting Helpers (Zero dynamic heap allocation)
// -----------------------------------------------------------------------------
fn format_time<'a>(total_secs: u32, buf: &'a mut [u8; 8]) -> &'a str {
  let mins = (total_secs / 60) % 100;
  let secs = total_secs % 60;
  buf[0] = b'0' + (mins / 10) as u8;
  buf[1] = b'0' + (mins % 10) as u8;
  buf[2] = b':';
  buf[3] = b'0' + (secs / 10) as u8;
  buf[4] = b'0' + (secs % 10) as u8;
  core::str::from_utf8(&buf[..5]).unwrap_or("00:00")
}
