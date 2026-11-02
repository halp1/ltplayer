//! USB bring-up: an MTP device backed by a 64 KB RAM disk.
//!
//! Connect the Nucleo's user USB-C port (not the ST-LINK port). The device
//! appears as "ltplayer" in Windows Explorer, Linux file managers, or OpenMTP
//! on macOS, with a `Music` folder and a `hello.txt`.
#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::rcc::{self, Hsi48Config, mux::Iclksel};
use embassy_stm32::usb::{self, Driver};
use embassy_stm32::{bind_interrupts, peripherals};
use embassy_usb::msos::windows_version;
use heapless::{String, Vec};
use mtp::usb::{UsbState, UsbTransport};
use mtp::{DeviceStrings, EntryMeta, Name, Responder, State, Storage, StorageError};
use panic_probe as _;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
  OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

const STRINGS: DeviceStrings = DeviceStrings {
  manufacturer: "ltplayer",
  model: "ltplayer",
  version: env!("CARGO_PKG_VERSION"),
  serial: "0001",
  friendly_name: "ltplayer",
  storage_label: "RAM disk",
};

const DISK: usize = 64 * 1024;
const MAX_NODES: usize = 32;

struct Node {
  path: String<128>,
  is_dir: bool,
  start: usize,
  len: usize,
}

/// Append-only RAM disk. Deleted file bytes are not reclaimed, which is fine
/// for a bring-up test.
struct RamDisk {
  bytes: &'static mut [u8; DISK],
  used: usize,
  nodes: Vec<Node, MAX_NODES>,
  listing: Option<(String<128>, usize)>,
  writing: Option<(usize, bool)>,
}

fn parent(path: &str) -> &str {
  match path.rfind('/') {
    Some(0) | None => "/",
    Some(i) => &path[..i],
  }
}

fn path_of(s: &str) -> Result<String<128>, StorageError> {
  s.try_into().map_err(|_| StorageError::Io)
}

impl RamDisk {
  fn find(&self, path: &str) -> Option<usize> {
    self.nodes.iter().position(|n| n.path == path)
  }

  fn is_dir(&self, path: &str) -> bool {
    path == "/" || self.find(path).is_some_and(|i| self.nodes[i].is_dir)
  }

  fn add(&mut self, path: &str, is_dir: bool) -> Result<usize, StorageError> {
    if !self.is_dir(parent(path)) {
      return Err(StorageError::NotFound);
    }
    let node = Node {
      path: path_of(path)?,
      is_dir,
      start: self.used,
      len: 0,
    };
    self.nodes.push(node).map_err(|_| StorageError::Full)?;
    Ok(self.nodes.len() - 1)
  }

  fn seed(&mut self, path: &str, body: &[u8]) {
    let i = self.add(path, false).unwrap();
    self.bytes[self.used..self.used + body.len()].copy_from_slice(body);
    self.used += body.len();
    self.nodes[i].len = body.len();
  }
}

impl Storage for RamDisk {
  async fn open_dir(&mut self, dir: &str) -> Result<(), StorageError> {
    if !self.is_dir(dir) {
      return Err(StorageError::NotFound);
    }
    self.listing = Some((path_of(dir)?, 0));
    Ok(())
  }

  async fn next_entry(&mut self, name: &mut Name) -> Result<Option<EntryMeta>, StorageError> {
    let Some((dir, pos)) = self.listing.as_mut() else {
      return Ok(None);
    };
    while *pos < self.nodes.len() {
      let n = &self.nodes[*pos];
      *pos += 1;
      if parent(&n.path) == dir.as_str() && n.path != "/" {
        name.clear();
        let _ = name.push_str(n.path.rsplit('/').next().unwrap_or(""));
        return Ok(Some(EntryMeta {
          is_dir: n.is_dir,
          size: n.len as u64,
        }));
      }
    }
    Ok(None)
  }

  async fn read(&mut self, path: &str, offset: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
    let n = &self.nodes[self.find(path).ok_or(StorageError::NotFound)?];
    let off = (offset as usize).min(n.len);
    let len = buf.len().min(n.len - off);
    buf[..len].copy_from_slice(&self.bytes[n.start + off..n.start + off + len]);
    Ok(len)
  }

  async fn create(&mut self, path: &str, _size: u64) -> Result<(), StorageError> {
    self.listing = None;
    if let Some(i) = self.find(path) {
      if self.nodes[i].is_dir {
        return Err(StorageError::Exists);
      }
      self.nodes.remove(i);
    }
    let i = self.add(path, false)?;
    self.writing = Some((i, false));
    Ok(())
  }

  async fn write(&mut self, data: &[u8]) -> Result<(), StorageError> {
    let (i, _) = self.writing.ok_or(StorageError::Io)?;
    if self.used + data.len() > DISK {
      return Err(StorageError::Full);
    }
    self.bytes[self.used..self.used + data.len()].copy_from_slice(data);
    self.used += data.len();
    self.nodes[i].len += data.len();
    Ok(())
  }

  async fn finish(&mut self) -> Result<(), StorageError> {
    self.writing.as_mut().ok_or(StorageError::Io)?.1 = true;
    Ok(())
  }

  async fn abort(&mut self) -> Result<(), StorageError> {
    if let Some((i, false)) = self.writing.take() {
      self.nodes.remove(i);
    }
    Ok(())
  }

  async fn remove(&mut self, path: &str) -> Result<(), StorageError> {
    self.listing = None;
    let i = self.find(path).ok_or(StorageError::NotFound)?;
    if self.nodes.iter().any(|n| parent(&n.path) == path) {
      return Err(StorageError::NotEmpty);
    }
    self.nodes.remove(i);
    Ok(())
  }

  async fn rename(&mut self, from: &str, to: &str) -> Result<(), StorageError> {
    self.listing = None;
    if self.find(to).is_some() {
      return Err(StorageError::Exists);
    }
    if !self.is_dir(parent(to)) {
      return Err(StorageError::NotFound);
    }
    let mut found = false;
    for n in self.nodes.iter_mut() {
      let inside = n.path.len() > from.len()
        && n.path.starts_with(from)
        && n.path.as_bytes()[from.len()] == b'/';
      if n.path == from || inside {
        let mut p = path_of(to)?;
        p.push_str(&n.path[from.len()..])
          .map_err(|_| StorageError::Io)?;
        n.path = p;
        found = true;
      }
    }
    if found {
      Ok(())
    } else {
      Err(StorageError::NotFound)
    }
  }

  async fn mkdir(&mut self, path: &str) -> Result<(), StorageError> {
    self.listing = None;
    if self.find(path).is_some() {
      return Err(StorageError::Exists);
    }
    self.add(path, true).map(|_| ())
  }

  fn capacity(&self) -> (u64, u64) {
    (DISK as u64, (DISK - self.used) as u64)
  }
}

static EP_OUT: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static USB_STATE: StaticCell<UsbState<'static>> = StaticCell::new();
static MTP_STATE: StaticCell<State<256>> = StaticCell::new();
static DISK_BYTES: StaticCell<[u8; DISK]> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
  let mut config = embassy_stm32::Config::default();
  config.rcc.hsi48 = Some(Hsi48Config {
    sync_from_usb: true,
  });
  config.rcc.mux.iclksel = Iclksel::HSI48;
  config.rcc.msis = Some(rcc::MSIRange::RANGE_48MHZ);
  let p = embassy_stm32::init(config);

  let mut usb_config = usb::Config::default();
  // Nucleo boards cannot sense VBUS on PA9 without a solder bridge.
  usb_config.vbus_detection = false;
  let driver = Driver::new_fs(
    p.USB_OTG_FS,
    Irqs,
    p.PA12,
    p.PA11,
    EP_OUT.init([0; 256]),
    usb_config,
  );

  // TODO: replace the pid.codes test VID:PID before release.
  let mut config = embassy_usb::Config::new(0x1209, 0x0001);
  config.manufacturer = Some(STRINGS.manufacturer);
  config.product = Some(STRINGS.model);
  config.serial_number = Some(STRINGS.serial);
  config.max_power = 100;

  let mut builder = embassy_usb::Builder::new(
    driver,
    config,
    CONFIG_DESC.init([0; 256]),
    BOS_DESC.init([0; 256]),
    MSOS_DESC.init([0; 256]),
    CONTROL_BUF.init([0; 64]),
  );
  builder.msos_descriptor(windows_version::WIN8_1, 0x20);
  let transport = UsbTransport::new(&mut builder, USB_STATE.init(UsbState::new()));
  let control = transport.control();
  let mut device = builder.build();

  let mut disk = RamDisk {
    bytes: DISK_BYTES.init([0; DISK]),
    used: 0,
    nodes: Vec::new(),
    listing: None,
    writing: None,
  };
  disk.mkdir("/Music").await.unwrap();
  disk.seed("/hello.txt", b"Hello from ltplayer over MTP.\n");

  let mut responder = Responder::new(transport, disk, MTP_STATE.init(State::new(STRINGS)));

  let mtp = async {
    loop {
      responder.transport().wait_connected().await;
      defmt::info!("mtp: host connected");
      loop {
        if control.take_session_reset() {
          responder.reset();
          let _ = responder.storage().abort().await;
        }
        if let Err(e) = responder.serve_one().await {
          defmt::info!("mtp: endpoint {}", e);
          responder.reset();
          let _ = responder.storage().abort().await;
          break;
        }
      }
    }
  };

  join(device.run(), mtp).await;
  unreachable!()
}
