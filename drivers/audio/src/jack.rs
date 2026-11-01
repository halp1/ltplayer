use embedded_hal::digital::OutputPin;

use crate::cs43131::{ConnectionType, OutputMode};

/// Picks the active output from the two jack detect switches.
///
/// The most recently plugged jack wins. If both appear at once (e.g. at boot),
/// the 4.4mm jack wins. Unplugging the active jack falls back to the other one
/// if it is still plugged.
#[derive(Debug, Default)]
pub struct JackSelector {
  plugged_35: bool,
  plugged_44: bool,
  active: Option<ConnectionType>,
}

impl JackSelector {
  pub const fn new() -> Self {
    Self {
      plugged_35: false,
      plugged_44: false,
      active: None,
    }
  }

  pub fn update(&mut self, plugged_35: bool, plugged_44: bool) -> OutputMode {
    let rose_35 = plugged_35 && !self.plugged_35;
    let rose_44 = plugged_44 && !self.plugged_44;
    self.plugged_35 = plugged_35;
    self.plugged_44 = plugged_44;

    let is_plugged = |c: ConnectionType| match c {
      ConnectionType::MM35 => plugged_35,
      ConnectionType::MM44 => plugged_44,
    };

    self.active = if rose_44 {
      Some(ConnectionType::MM44)
    } else if rose_35 {
      Some(ConnectionType::MM35)
    } else if self.active.is_some_and(is_plugged) {
      self.active
    } else if plugged_44 {
      Some(ConnectionType::MM44)
    } else if plugged_35 {
      Some(ConnectionType::MM35)
    } else {
      None
    };

    self.mode()
  }

  pub fn mode(&self) -> OutputMode {
    match self.active {
      Some(c) => OutputMode::Connected(c),
      None => OutputMode::Disconnected,
    }
  }
}

/// Drives the shared SEL line of the TMUX4827 switches.
pub struct JackRouter<SEL: OutputPin> {
  sel: SEL,
}

impl<SEL: OutputPin> JackRouter<SEL> {
  pub fn new(sel: SEL) -> Self {
    Self { sel }
  }

  pub fn route(&mut self, connection: ConnectionType) -> Result<(), SEL::Error> {
    // TODO(pcb): confirm SEL polarity against the schematic.
    match connection {
      ConnectionType::MM35 => self.sel.set_low(),
      ConnectionType::MM44 => self.sel.set_high(),
    }
  }
}
