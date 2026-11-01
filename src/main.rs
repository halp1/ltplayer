#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::SDMMC1;
use embassy_stm32::sdmmc::InterruptHandler;
use embassy_stm32::{spi, time::Hertz};
use embassy_time::Timer;
use panic_probe as _;
use static_cell::StaticCell;

pub mod audio;
pub mod controls;
pub mod file_manager;
pub mod jack;
pub mod playback;
pub mod renderer;
pub mod state;

bind_interrupts!(struct Irqs {
  SDMMC1 => InterruptHandler<SDMMC1>;
});

static SDMMC: StaticCell<embassy_stm32::sdmmc::Sdmmc<'static>> = StaticCell::new();
static FS_CELL: StaticCell<file_manager::FS<'static, 'static>> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
  let mut config = embassy_stm32::Config::default();
  config.rcc.msis = Some(embassy_stm32::rcc::MSIRange::RANGE_48MHZ);
  config.rcc.msik = Some(embassy_stm32::rcc::MSIRange::RANGE_48MHZ);
  let p = embassy_stm32::init(config);

  let sdmmc = SDMMC.init(embassy_stm32::sdmmc::Sdmmc::new_4bit(
    p.SDMMC1,
    Irqs,
    p.PC12,
    p.PD2,
    p.PC8,
    p.PC9,
    p.PC10,
    p.PC11,
    Default::default(),
  ));

  let fs = FS_CELL.init(
    microsd::mount(sdmmc, microsd::SdCardConfig::default())
      .await
      .expect("SD card mount failed"),
  );

  defmt::info!("sd up");

  let _touch_cs = Output::new(p.PF13, Level::High, Speed::Low);
  let _sd_cs = Output::new(p.PF14, Level::High, Speed::Low);
  let _memory_cs = Output::new(p.PE9, Level::High, Speed::Low);

  let cs = Output::new(p.PD14, Level::High, Speed::High);
  let dc = Output::new(p.PD15, Level::Low, Speed::High);
  let rst = Output::new(p.PF12, Level::High, Speed::High);
  let _backlight = Output::new(p.PE11, Level::High, Speed::Low);

  let mut display_spi_config = spi::Config::default();
  display_spi_config.frequency = Hertz::mhz(24);
  let spi = spi::Spi::new_blocking(p.SPI1, p.PA5, p.PA7, p.PA6, display_spi_config);

  let mut display_buffer = [0_u8; 2048];
  let display = display::new(spi, cs, dc, rst, &mut display_buffer).expect("Display init failed");

  defmt::info!("display up");

  let files = file_manager::list_files(fs)
    .await
    .expect("File listing failed");

  // TODO(pcb): placeholder pins until the PCB pinout is final. PE2 to PE6 are
  // kept free for SAI1 and PB8/PB9 for the DAC I2C.
  let jack_sel = Output::new(p.PG0, Level::Low, Speed::Low);
  let detect_35 = Input::new(p.PG1, Pull::Up);
  let detect_44 = Input::new(p.PG2, Pull::Up);

  spawner.spawn(jack::jack_task(detect_35, detect_44).unwrap());
  spawner.spawn(playback::playback_task(fs, files.clone(), jack_sel).unwrap());

  spawner.spawn(
    controls::controls_task(
      files.clone(),
      Input::new(p.PA3, Pull::Up),
      Input::new(p.PA2, Pull::Up),
      Input::new(p.PC3, Pull::Up),
      Input::new(p.PB0, Pull::Up),
    )
    .unwrap(),
  );

  let mut renderer = renderer::Renderer::new(display);

  defmt::info!("ready to start");

  let mut last_state: Option<state::State> = None;

  loop {
    let current_state = { *state::STATE.lock().await };

    match (last_state, current_state) {
      (None, state::State::Menu(idx)) => {
        renderer.clear().unwrap();
        renderer.render_file_list(files.as_slice(), idx).unwrap();
        last_state = Some(current_state);
      }
      (Some(state::State::Menu(prev_idx)), state::State::Menu(curr_idx)) => {
        if prev_idx != curr_idx {
          renderer.render_file_list(files.as_slice(), curr_idx).unwrap();
          last_state = Some(current_state);
        }
      }
      (Some(state::State::Menu(_)), state::State::Playing(p))
      | (None, state::State::Playing(p)) => {
        renderer.clear().unwrap();
        renderer
          .render_player(files[p.index].as_str(), p.elapsed, p.total, p.is_playing)
          .unwrap();
        last_state = Some(current_state);
      }
      (Some(state::State::Playing(_)), state::State::Menu(idx)) => {
        renderer.clear().unwrap();
        renderer.render_file_list(files.as_slice(), idx).unwrap();
        last_state = Some(current_state);
      }
      (Some(state::State::Playing(prev_p)), state::State::Playing(curr_p)) => {
        if prev_p.index != curr_p.index {
          renderer.clear().unwrap();
          renderer
            .render_player(
              files[curr_p.index].as_str(),
              curr_p.elapsed,
              curr_p.total,
              curr_p.is_playing,
            )
            .unwrap();
        } else {
          if prev_p.elapsed != curr_p.elapsed {
            renderer
              .update_progress(curr_p.elapsed, curr_p.total)
              .unwrap();
          }
          if prev_p.is_playing != curr_p.is_playing {
            renderer.update_play_pause(curr_p.is_playing).unwrap();
          }
        }
        last_state = Some(current_state);
      }
    }

    Timer::after_millis(30).await;
  }
}
