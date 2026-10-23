use embedded_graphics::{
  mono_font::{
    ascii::{FONT_6X10, FONT_7X13, FONT_9X15},
    MonoTextStyle,
  },
  pixelcolor::Rgb565,
  prelude::*,
  primitives::{Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Triangle},
  text::{Alignment, Text},
};

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

pub struct PlayerState<'a> {
  pub track_name: &'a str,
  pub elapsed_secs: u32,
  pub total_secs: u32,
  pub is_playing: bool,
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

  // ===========================================================================
  // SCREEN 1: File List (Playlist Browser)
  // ===========================================================================
  /// Render the scrollable file list screen.
  ///
  /// - Displays 7 items on screen at a time.
  /// - Automatically scrolls the view window so `selected_index` is always visible.
  /// - Highlights the active item with an accent bar and pointer arrow.
  pub fn render_file_list(
    &mut self,
    files: &[&str],
    selected_index: usize,
  ) -> Result<(), T::Error> {
    // 1. Clear background
    self.display.clear(COLOR_BG)?;

    // 2. Render List Items
    const VISIBLE_COUNT: usize = 9;
    const ITEM_HEIGHT: i32 = 25;
    const LIST_TOP: i32 = 8;

    if files.is_empty() {
      let empty_style = MonoTextStyle::new(&FONT_7X13, COLOR_TEXT_MUTED);
      Text::with_alignment(
        "No MP3 files found",
        Point::new(120, 110),
        empty_style,
        Alignment::Center,
      )
      .draw(&mut self.display)?;
    } else {
      // Calculate scrolling window offset
      let window_start = if selected_index >= VISIBLE_COUNT {
        selected_index - VISIBLE_COUNT + 1
      } else {
        0
      };

      for slot in 0..VISIBLE_COUNT {
        let file_idx = window_start + slot;
        if file_idx >= files.len() {
          break;
        }

        let y = LIST_TOP + (slot as i32 * ITEM_HEIGHT);
        let is_selected = file_idx == selected_index;

        if is_selected {
          // Highlight card background for selected item
          Rectangle::new(Point::new(4, y), Size::new(232, 23))
            .into_styled(
              PrimitiveStyleBuilder::new()
                .fill_color(COLOR_SURFACE)
                .stroke_color(COLOR_ACCENT)
                .stroke_width(1)
                .build(),
            )
            .draw(&mut self.display)?;

          // Indicator pointer: ">"
          let arrow_style = MonoTextStyle::new(&FONT_7X13, COLOR_ACCENT);
          Text::new(">", Point::new(10, y + 16), arrow_style).draw(&mut self.display)?;

          // Selected track text
          let sel_style = MonoTextStyle::new(&FONT_7X13, COLOR_TEXT_PRIMARY);
          Text::new(files[file_idx], Point::new(24, y + 16), sel_style)
            .draw(&mut self.display)?;
        } else {
          // Unselected track text (dimmer)
          let unsel_style = MonoTextStyle::new(&FONT_7X13, COLOR_TEXT_MUTED);
          Text::new(files[file_idx], Point::new(24, y + 16), unsel_style)
            .draw(&mut self.display)?;
        }
      }
    }

    Ok(())
  }

  // ===========================================================================
  // SCREEN 2: Now Playing (Player & Artwork)
  // ===========================================================================
  /// Render the player screen with album art placeholder, track name, play/pause icon, and seekbar.
  pub fn render_player(&mut self, state: &PlayerState) -> Result<(), T::Error> {
    // 1. Clear background
    self.display.clear(COLOR_BG)?;

    // 2. Album Art Placeholder Card (Centered 96x96 box)
    const ART_X: i32 = 72;
    const ART_Y: i32 = 16;
    const ART_SIZE: u32 = 96;

    Rectangle::new(Point::new(ART_X, ART_Y), Size::new(ART_SIZE, ART_SIZE))
      .into_styled(
        PrimitiveStyleBuilder::new()
          .fill_color(COLOR_SURFACE)
          .stroke_color(COLOR_BORDER)
          .stroke_width(1)
          .build(),
      )
      .draw(&mut self.display)?;

    // Draw eighth notes (♫): perfectly centered (25px margins on left/right, 20px/22px top/bottom)
    let note_head_style = PrimitiveStyle::with_fill(COLOR_ACCENT);
    let note_line_style = PrimitiveStyle::with_stroke(COLOR_ACCENT, 2);

    // Left note head & stem
    Circle::new(Point::new(ART_X + 25, ART_Y + 58), 16)
      .into_styled(note_head_style)
      .draw(&mut self.display)?;
    Line::new(
      Point::new(ART_X + 39, ART_Y + 66),
      Point::new(ART_X + 39, ART_Y + 28),
    )
    .into_styled(note_line_style)
    .draw(&mut self.display)?;

    // Right note head & stem
    Circle::new(Point::new(ART_X + 55, ART_Y + 48), 16)
      .into_styled(note_head_style)
      .draw(&mut self.display)?;
    Line::new(
      Point::new(ART_X + 69, ART_Y + 56),
      Point::new(ART_X + 69, ART_Y + 20),
    )
    .into_styled(note_line_style)
    .draw(&mut self.display)?;

    // Cross beam connecting both stems
    Line::new(
      Point::new(ART_X + 39, ART_Y + 28),
      Point::new(ART_X + 69, ART_Y + 20),
    )
    .into_styled(PrimitiveStyle::with_stroke(COLOR_ACCENT, 4))
    .draw(&mut self.display)?;

    // 3. Track File Name (Centered below art)
    let track_style = MonoTextStyle::new(&FONT_9X15, COLOR_TEXT_PRIMARY);
    Text::with_alignment(
      state.track_name,
      Point::new(120, 136),
      track_style,
      Alignment::Center,
    )
    .draw(&mut self.display)?;

    // 4. Seek / Progress Bar (X: 20 to 220, width = 200px)
    const BAR_X: i32 = 20;
    const BAR_Y: i32 = 162;
    const BAR_WIDTH: u32 = 200;
    const BAR_HEIGHT: u32 = 6;

    // Background track
    Rectangle::new(Point::new(BAR_X, BAR_Y), Size::new(BAR_WIDTH, BAR_HEIGHT))
      .into_styled(PrimitiveStyle::with_fill(COLOR_SURFACE))
      .draw(&mut self.display)?;

    // Filled progress
    let fill_width = if state.total_secs > 0 {
      ((state.elapsed_secs.min(state.total_secs) as u64 * BAR_WIDTH as u64)
        / state.total_secs as u64) as u32
    } else {
      0
    };

    if fill_width > 0 {
      Rectangle::new(Point::new(BAR_X, BAR_Y), Size::new(fill_width, BAR_HEIGHT))
        .into_styled(PrimitiveStyle::with_fill(COLOR_ACCENT))
        .draw(&mut self.display)?;
    }

    // Time labels: "01:23" on left, "03:45" on right (Below Seek Bar)
    let time_style = MonoTextStyle::new(&FONT_6X10, COLOR_TEXT_MUTED);
    let mut time_buf_a = [0u8; 8];
    let elapsed_str = format_time(state.elapsed_secs, &mut time_buf_a);
    Text::new(elapsed_str, Point::new(BAR_X, BAR_Y + 16), time_style)
      .draw(&mut self.display)?;

    let mut time_buf_b = [0u8; 8];
    let total_str = format_time(state.total_secs, &mut time_buf_b);
    Text::with_alignment(
      total_str,
      Point::new(BAR_X + BAR_WIDTH as i32, BAR_Y + 16),
      time_style,
      Alignment::Right,
    )
    .draw(&mut self.display)?;

    // 5. Play / Pause Status Icon (Centered UNDER seekbar/timestamps, clean white/monochrome)
    if state.is_playing {
      // Pause Bars (❚❚)
      Rectangle::new(Point::new(115, 196), Size::new(3, 16))
        .into_styled(PrimitiveStyle::with_fill(COLOR_TEXT_PRIMARY))
        .draw(&mut self.display)?;
      Rectangle::new(Point::new(122, 196), Size::new(3, 16))
        .into_styled(PrimitiveStyle::with_fill(COLOR_TEXT_PRIMARY))
        .draw(&mut self.display)?;
    } else {
      // Play Triangle (▶)
      Triangle::new(
        Point::new(115, 195),
        Point::new(115, 213),
        Point::new(129, 204),
      )
      .into_styled(PrimitiveStyle::with_fill(COLOR_TEXT_PRIMARY))
      .draw(&mut self.display)?;
    }

    Ok(())
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
