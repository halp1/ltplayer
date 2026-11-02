//! Host tests: the responder driven by a fake host over an in-memory disk.

use std::collections::{BTreeMap, VecDeque};
use std::string::{String as StdString, ToString};
use std::vec::Vec as StdVec;

use embassy_futures::block_on;

use crate::codes::{STORAGE_ID, format, op, prop, resp};
use crate::container::{Command, ContainerError, HEADER_LEN, Header, kind};
use crate::dataset::{Name, Sink, parse_str};
use crate::storage::{EntryMeta, Storage, StorageError};
use crate::transport::Transport;
use crate::{DeviceStrings, Responder, State};

const MP: usize = 64;
const N: usize = 64;

const STRINGS: DeviceStrings = DeviceStrings {
  manufacturer: "ltplayer",
  model: "ltplayer",
  version: "0.1.0",
  serial: "0001",
  friendly_name: "ltplayer",
  storage_label: "SD card",
};

#[derive(Debug, Clone, PartialEq)]
enum Node {
  File(StdVec<u8>),
  Dir,
}

#[derive(Default)]
struct MemDisk {
  nodes: BTreeMap<StdString, Node>,
  listing: VecDeque<(StdString, EntryMeta)>,
  writing: Option<(StdString, bool)>,
  capacity: u64,
}

impl MemDisk {
  fn new() -> Self {
    let mut d = Self {
      capacity: 1 << 20,
      ..Default::default()
    };
    d.nodes.insert("/".into(), Node::Dir);
    d
  }

  fn with(mut self, path: &str, node: Node) -> Self {
    self.nodes.insert(path.into(), node);
    self
  }

  fn parent(path: &str) -> StdString {
    match path.rfind('/') {
      Some(0) | None => "/".into(),
      Some(i) => path[..i].into(),
    }
  }

  fn children(&self, dir: &str) -> impl Iterator<Item = (&StdString, &Node)> {
    let dir = dir.to_string();
    self
      .nodes
      .iter()
      .filter(move |(p, _)| p.as_str() != "/" && Self::parent(p) == dir)
  }

  fn used(&self) -> u64 {
    self
      .nodes
      .values()
      .map(|n| match n {
        Node::File(b) => b.len() as u64,
        Node::Dir => 0,
      })
      .sum()
  }

  fn file(&self, path: &str) -> Option<&[u8]> {
    match self.nodes.get(path) {
      Some(Node::File(b)) => Some(b),
      _ => None,
    }
  }
}

impl Storage for MemDisk {
  async fn open_dir(&mut self, dir: &str) -> Result<(), StorageError> {
    if self.nodes.get(dir) != Some(&Node::Dir) {
      return Err(StorageError::NotFound);
    }
    self.listing = self
      .children(dir)
      .map(|(p, n)| {
        let name = p.rsplit('/').next().unwrap().to_string();
        let meta = match n {
          Node::Dir => EntryMeta {
            is_dir: true,
            size: 0,
          },
          Node::File(b) => EntryMeta {
            is_dir: false,
            size: b.len() as u64,
          },
        };
        (name, meta)
      })
      .collect();
    Ok(())
  }

  async fn next_entry(&mut self, name: &mut Name) -> Result<Option<EntryMeta>, StorageError> {
    Ok(self.listing.pop_front().map(|(n, m)| {
      name.clear();
      name.push_str(&n).unwrap();
      m
    }))
  }

  async fn read(&mut self, path: &str, offset: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
    let data = self.file(path).ok_or(StorageError::NotFound)?;
    let start = (offset as usize).min(data.len());
    let n = buf.len().min(data.len() - start);
    buf[..n].copy_from_slice(&data[start..start + n]);
    Ok(n)
  }

  async fn create(&mut self, path: &str, _size: u64) -> Result<(), StorageError> {
    if self.nodes.get(path) == Some(&Node::Dir) {
      return Err(StorageError::Exists);
    }
    if self.nodes.get(&Self::parent(path)) != Some(&Node::Dir) {
      return Err(StorageError::NotFound);
    }
    self.nodes.insert(path.into(), Node::File(StdVec::new()));
    self.writing = Some((path.into(), false));
    Ok(())
  }

  async fn write(&mut self, data: &[u8]) -> Result<(), StorageError> {
    if self.used() + data.len() as u64 > self.capacity {
      return Err(StorageError::Full);
    }
    let (path, _) = self.writing.as_ref().ok_or(StorageError::Io)?;
    match self.nodes.get_mut(path) {
      Some(Node::File(b)) => b.extend_from_slice(data),
      _ => return Err(StorageError::Io),
    }
    Ok(())
  }

  async fn finish(&mut self) -> Result<(), StorageError> {
    let w = self.writing.as_mut().ok_or(StorageError::Io)?;
    w.1 = true;
    Ok(())
  }

  async fn abort(&mut self) -> Result<(), StorageError> {
    if let Some((path, false)) = self.writing.take() {
      self.nodes.remove(&path);
    }
    Ok(())
  }

  async fn remove(&mut self, path: &str) -> Result<(), StorageError> {
    if self.children(path).next().is_some() {
      return Err(StorageError::NotEmpty);
    }
    self
      .nodes
      .remove(path)
      .map(|_| ())
      .ok_or(StorageError::NotFound)
  }

  async fn rename(&mut self, from: &str, to: &str) -> Result<(), StorageError> {
    if self.nodes.contains_key(to) {
      return Err(StorageError::Exists);
    }
    let moved: StdVec<StdString> = self
      .nodes
      .keys()
      .filter(|p| p.as_str() == from || p.starts_with(&(from.to_string() + "/")))
      .cloned()
      .collect();
    if moved.is_empty() {
      return Err(StorageError::NotFound);
    }
    for p in moved {
      let node = self.nodes.remove(&p).unwrap();
      self.nodes.insert(to.to_string() + &p[from.len()..], node);
    }
    Ok(())
  }

  async fn mkdir(&mut self, path: &str) -> Result<(), StorageError> {
    if self.nodes.contains_key(path) {
      return Err(StorageError::Exists);
    }
    self.nodes.insert(path.into(), Node::Dir);
    Ok(())
  }

  fn capacity(&self) -> (u64, u64) {
    (self.capacity, self.capacity - self.used())
  }
}

#[derive(Debug, PartialEq)]
struct Closed;

/// Fake host side of the bulk pipes.
#[derive(Default)]
struct Pipe {
  to_device: VecDeque<StdVec<u8>>,
  from_device: StdVec<StdVec<u8>>,
  cancel_after_writes: Option<usize>,
}

impl Transport for &mut Pipe {
  type Error = Closed;
  const MAX_PACKET: usize = MP;

  async fn read_packet(&mut self, buf: &mut [u8]) -> Result<usize, Closed> {
    let p = self.to_device.pop_front().ok_or(Closed)?;
    buf[..p.len()].copy_from_slice(&p);
    Ok(p.len())
  }

  async fn write_packet(&mut self, data: &[u8]) -> Result<(), Closed> {
    self.from_device.push(data.to_vec());
    Ok(())
  }

  fn take_cancel(&mut self) -> bool {
    self
      .cancel_after_writes
      .is_some_and(|n| self.from_device.len() >= n)
  }
}

fn command(code: u16, tx: u32, params: &[u32]) -> StdVec<u8> {
  let len = HEADER_LEN + 4 * params.len();
  let mut v = Header {
    length: len as u32,
    kind: kind::COMMAND,
    code,
    transaction_id: tx,
  }
  .encode()
  .to_vec();
  for p in params {
    v.extend_from_slice(&p.to_le_bytes());
  }
  v
}

/// Split a data container into packets the way a host would.
fn data_packets(code: u16, tx: u32, payload: &[u8]) -> StdVec<StdVec<u8>> {
  let mut all = Header {
    length: (HEADER_LEN + payload.len()) as u32,
    kind: kind::DATA,
    code,
    transaction_id: tx,
  }
  .encode()
  .to_vec();
  all.extend_from_slice(payload);
  let mut out: StdVec<StdVec<u8>> = all.chunks(MP).map(<[u8]>::to_vec).collect();
  if all.len().is_multiple_of(MP) {
    out.push(StdVec::new());
  }
  out
}

struct Bytes(StdVec<u8>);

impl Sink for Bytes {
  type Err = core::convert::Infallible;
  async fn put(&mut self, b: &[u8]) -> Result<(), Self::Err> {
    self.0.extend_from_slice(b);
    Ok(())
  }
}

fn ptp_str(s: &str) -> StdVec<u8> {
  let mut b = Bytes(StdVec::new());
  block_on(b.str(s)).unwrap();
  b.0
}

fn object_info(name: &str, fmt: u16, size: u32) -> StdVec<u8> {
  let mut v = StdVec::new();
  v.extend_from_slice(&STORAGE_ID.to_le_bytes());
  v.extend_from_slice(&fmt.to_le_bytes());
  v.extend_from_slice(&0u16.to_le_bytes());
  v.extend_from_slice(&size.to_le_bytes());
  v.resize(52, 0);
  v.extend_from_slice(&ptp_str(name));
  v.extend_from_slice(&[0, 0, 0]);
  v
}

#[derive(Debug)]
struct Done {
  code: u16,
  params: StdVec<u32>,
  data: StdVec<u8>,
}

/// One device under test with a session-scoped transaction counter.
struct Host {
  pipe: Pipe,
  disk: MemDisk,
  state: Box<State<N>>,
  tx: u32,
}

impl Host {
  fn new(disk: MemDisk) -> Self {
    let mut h = Self {
      pipe: Pipe::default(),
      disk,
      state: Box::new(State::new(STRINGS)),
      tx: 0,
    };
    assert_eq!(h.call(op::OPEN_SESSION, &[1], None).code, resp::OK);
    h
  }

  fn call(&mut self, code: u16, params: &[u32], data: Option<&[u8]>) -> Done {
    self.tx += 1;
    self
      .pipe
      .to_device
      .push_back(command(code, self.tx, params));
    if let Some(d) = data {
      self.pipe.to_device.extend(data_packets(code, self.tx, d));
    }
    self.pipe.from_device.clear();
    self.run();
    parse_reply(&self.pipe.from_device)
  }

  fn run(&mut self) {
    let mut r = Responder::new(&mut self.pipe, &mut self.disk, &mut *self.state);
    while block_on(r.serve_one()).is_ok() {}
  }

  fn handles(&mut self, parent: u32) -> StdVec<u32> {
    let d = self.call(op::GET_OBJECT_HANDLES, &[STORAGE_ID, 0, parent], None);
    assert_eq!(d.code, resp::OK);
    u32s(&d.data[4..])
  }

  fn name_of(&mut self, h: u32) -> StdString {
    let d = self.call(
      op::GET_OBJECT_PROP_VALUE,
      &[h, prop::OBJECT_FILE_NAME as u32],
      None,
    );
    assert_eq!(d.code, resp::OK, "name of {h}");
    let mut n = Name::new();
    parse_str(&d.data, &mut n).unwrap();
    n.as_str().into()
  }

  fn find(&mut self, parent: u32, name: &str) -> u32 {
    for h in self.handles(parent) {
      if self.name_of(h) == name {
        return h;
      }
    }
    panic!("{name} not found")
  }
}

fn u32s(b: &[u8]) -> StdVec<u32> {
  b.as_chunks::<4>()
    .0
    .iter()
    .map(|&c| u32::from_le_bytes(c))
    .collect()
}

/// Reassemble the device's data phase (if any) and its response.
fn parse_reply(packets: &[StdVec<u8>]) -> Done {
  let mut data = StdVec::new();
  let mut i = 0;
  if let Some(first) = packets.first() {
    let h = Header::parse(first).unwrap();
    if h.kind == kind::DATA {
      let mut all = first.clone();
      i = 1;
      while all.len() < h.length as usize {
        all.extend_from_slice(&packets[i]);
        i += 1;
      }
      if all.len() % MP == 0 {
        assert!(packets[i].is_empty(), "missing ZLP");
        i += 1;
      }
      assert_eq!(all.len(), h.length as usize, "data length matches header");
      data = all[HEADER_LEN..].to_vec();
    }
  }
  let r = packets.get(i).expect("no response");
  let h = Header::parse(r).unwrap();
  assert_eq!(h.kind, kind::RESPONSE);
  assert_eq!(packets.len(), i + 1, "nothing after the response");
  Done {
    code: h.code,
    params: u32s(&r[HEADER_LEN..h.length as usize]),
    data,
  }
}

fn music_disk() -> MemDisk {
  MemDisk::new()
    .with("/a.mp3", Node::File(vec![1; 10]))
    .with("/Albums", Node::Dir)
    .with("/Albums/One", Node::Dir)
    .with("/Albums/One/01 Intro.mp3", Node::File((0..200).collect()))
    .with("/Albums/One/02 Song.flac", Node::File(vec![7; 3]))
}

#[test]
fn container_parsing_rejects_malformed_commands() {
  let ok = command(op::OPEN_SESSION, 7, &[1]);
  let mut too_long = command(op::OPEN_SESSION, 1, &[1, 2, 3, 4, 5]);
  too_long.extend_from_slice(&[0; 4]);
  too_long[0] = 36;
  let mut odd = ok.clone();
  odd[0] = 15;
  let mut overrun = ok.clone();
  overrun[0] = 40;
  let mut data = ok.clone();
  data[4] = kind::DATA as u8;

  let cases: &[(&[u8], Result<(), ContainerError>)] = &[
    (&ok, Ok(())),
    (&ok[..8], Err(ContainerError::TooShort)),
    (&odd, Err(ContainerError::BadLength)),
    (&overrun, Err(ContainerError::BadLength)),
    (&too_long, Err(ContainerError::TooManyParams)),
    (&data, Err(ContainerError::WrongType)),
  ];
  for (bytes, want) in cases {
    assert_eq!(Command::parse(bytes).map(|_| ()), *want, "{bytes:?}");
  }
  let c = Command::parse(&ok).unwrap();
  assert_eq!(
    (c.code, c.transaction_id, c.params()),
    (op::OPEN_SESSION, 7, &[1u32][..])
  );
  assert_eq!(c.param(3), 0);
}

#[test]
fn ptp_strings_round_trip_and_reject_garbage() {
  let mut n = Name::new();
  for s in ["", "a.mp3", "Sigur Rós ( ) 🎵.flac"] {
    let enc = ptp_str(s);
    assert_eq!(parse_str(&enc, &mut n), Some(enc.len()));
    assert_eq!(n.as_str(), s);
  }
  // Count says 4 units but only 1 is present.
  assert_eq!(parse_str(&[4, b'a', 0], &mut n), None);
  // Lone surrogate.
  assert_eq!(parse_str(&[2, 0x00, 0xD8, 0, 0], &mut n), None);
}

#[test]
fn session_rules() {
  let mut h = Host::new(MemDisk::new());
  let cases: &[(u16, &[u32], u16)] = &[
    (op::OPEN_SESSION, &[2], resp::SESSION_ALREADY_OPEN),
    (
      op::GET_STORAGE_INFO,
      &[0x0002_0001],
      resp::INVALID_STORAGE_ID,
    ),
    (0x1010, &[], resp::OPERATION_NOT_SUPPORTED),
    (op::GET_OBJECT_INFO, &[999], resp::INVALID_OBJECT_HANDLE),
    (
      op::GET_OBJECT_PROP_LIST,
      &[0, 0, 0, 0, 0],
      resp::SPEC_BY_GROUP_UNSUPPORTED,
    ),
  ];
  for &(code, params, want) in cases {
    assert_eq!(h.call(code, params, None).code, want, "op {code:#06x}");
  }
  assert_eq!(h.call(op::CLOSE_SESSION, &[], None).code, resp::OK);
  assert_eq!(
    h.call(op::GET_STORAGE_IDS, &[], None).code,
    resp::SESSION_NOT_OPEN
  );
  // A data phase sent outside a session is drained, not parsed as a command.
  let info = object_info("x.mp3", format::MP3, 1);
  let d = h.call(op::SEND_OBJECT_INFO, &[STORAGE_ID, 0], Some(&info));
  assert_eq!(d.code, resp::SESSION_NOT_OPEN);
  assert!(h.pipe.to_device.is_empty());
}

#[test]
fn device_info_advertises_mtp_and_operations() {
  let mut h = Host::new(MemDisk::new());
  let d = h.call(op::GET_DEVICE_INFO, &[], None);
  assert_eq!(d.code, resp::OK);
  let mut s = Name::new();
  let used = parse_str(&d.data[8..], &mut s).unwrap();
  assert_eq!(s.as_str(), "microsoft.com: 1.0; ");
  let ops_at = 8 + used + 2;
  let n = u32::from_le_bytes(d.data[ops_at..ops_at + 4].try_into().unwrap()) as usize;
  let ops: StdVec<u16> = d.data[ops_at + 4..ops_at + 4 + 2 * n]
    .as_chunks::<2>()
    .0
    .iter()
    .map(|&c| u16::from_le_bytes(c))
    .collect();
  assert!(ops.contains(&op::SEND_OBJECT) && ops.contains(&op::GET_OBJECT_PROP_LIST));
}

#[test]
fn nested_listing_and_reads_cross_packet_boundaries() {
  let mut h = Host::new(music_disk());
  let root = h.handles(0xFFFF_FFFF);
  assert_eq!(root.len(), 2);
  let albums = h.find(0xFFFF_FFFF, "Albums");
  let one = h.find(albums, "One");
  let intro = h.find(one, "01 Intro.mp3");

  let info = h.call(op::GET_OBJECT_INFO, &[intro], None);
  assert_eq!(info.code, resp::OK);
  assert_eq!(
    u16::from_le_bytes([info.data[4], info.data[5]]),
    format::MP3
  );
  assert_eq!(
    u32::from_le_bytes(info.data[8..12].try_into().unwrap()),
    200
  );
  assert_eq!(
    u32::from_le_bytes(info.data[38..42].try_into().unwrap()),
    one
  );

  // Fresh responder state: handles are resolved by re-listing parents.
  h.state.forget_names();
  let whole = h.call(op::GET_OBJECT, &[intro], None);
  assert_eq!(whole.code, resp::OK);
  assert_eq!(whole.data, (0..200).collect::<StdVec<u8>>());

  // 52 payload bytes + 12 header = exactly one packet, so a ZLP must follow.
  let cases: &[(u32, u32, &[u8])] = &[
    (0, 52, &whole.data[..52]),
    (60, 100, &whole.data[60..160]),
    (190, 50, &whole.data[190..]),
    (200, 10, &[]),
  ];
  for &(off, len, want) in cases {
    let d = h.call(op::GET_PARTIAL_OBJECT, &[intro, off, len], None);
    assert_eq!(d.code, resp::OK, "offset {off}");
    assert_eq!(d.data, want, "offset {off}");
    assert_eq!(d.params, [want.len() as u32]);
  }
  assert_eq!(
    h.call(op::GET_PARTIAL_OBJECT, &[intro, 201, 1], None).code,
    resp::INVALID_PARAMETER
  );
  assert_eq!(
    h.call(op::GET_OBJECT, &[one], None).code,
    resp::INVALID_OBJECT_HANDLE
  );
}

#[test]
fn upload_then_read_back() {
  let mut h = Host::new(music_disk());
  let albums = h.find(0xFFFF_FFFF, "Albums");
  let body: StdVec<u8> = (0..=255u8).cycle().take(9000).collect();

  let d = h.call(
    op::SEND_OBJECT_INFO,
    &[STORAGE_ID, albums],
    Some(&object_info("new.mp3", format::MP3, body.len() as u32)),
  );
  assert_eq!(d.code, resp::OK);
  let [sid, parent, new] = d.params[..] else {
    panic!("{:?}", d.params)
  };
  assert_eq!((sid, parent), (STORAGE_ID, albums));
  assert!(!h.handles(albums).contains(&new), "hidden until SendObject");

  assert_eq!(h.call(op::SEND_OBJECT, &[], Some(&body)).code, resp::OK);
  assert_eq!(h.disk.file("/Albums/new.mp3"), Some(&body[..]));
  assert!(h.handles(albums).contains(&new));
  assert_eq!(h.call(op::GET_OBJECT, &[new], None).data, body);

  // A second SendObject without a new ObjectInfo is refused.
  assert_eq!(
    h.call(op::SEND_OBJECT, &[], Some(b"x")).code,
    resp::NO_VALID_OBJECT_INFO
  );
}

#[test]
fn rejected_uploads_leave_no_file() {
  let cases: &[(&str, u32, usize, u16)] = &[
    ("short.mp3", 100, 40, resp::INCOMPLETE_TRANSFER),
    ("big.mp3", 2 << 20, 0, resp::STORE_FULL),
    ("a/b.mp3", 1, 1, resp::INVALID_DATASET),
    ("..", 1, 1, resp::INVALID_DATASET),
  ];
  for &(name, announced, sent, want) in cases {
    let mut h = Host::new(MemDisk::new());
    let info = h.call(
      op::SEND_OBJECT_INFO,
      &[STORAGE_ID, 0xFFFF_FFFF],
      Some(&object_info(name, format::MP3, announced)),
    );
    let code = if info.code == resp::OK {
      h.call(op::SEND_OBJECT, &[], Some(&vec![0; sent])).code
    } else {
      info.code
    };
    assert_eq!(code, want, "{name}");
    assert_eq!(h.disk.nodes.len(), 1, "{name}: only the root is left");
    assert!(h.handles(0xFFFF_FFFF).is_empty(), "{name}");
  }
}

#[test]
fn host_cancel_mid_upload_removes_partial_file() {
  let mut h = Host::new(MemDisk::new());
  h.call(
    op::SEND_OBJECT_INFO,
    &[STORAGE_ID, 0xFFFF_FFFF],
    Some(&object_info("c.mp3", format::MP3, 9000)),
  );
  h.tx += 1;
  h.pipe
    .to_device
    .push_back(command(op::SEND_OBJECT, h.tx, &[]));
  h.pipe
    .to_device
    .extend(data_packets(op::SEND_OBJECT, h.tx, &[1; 9000]));
  h.pipe.from_device.clear();
  h.pipe.cancel_after_writes = Some(0);
  h.run();
  h.pipe.cancel_after_writes = None;
  assert!(h.pipe.from_device.is_empty(), "no response after Cancel");
  assert_eq!(h.disk.nodes.len(), 1);
}

#[test]
fn mkdir_rename_move_and_recursive_delete() {
  let mut h = Host::new(music_disk());
  let root = 0xFFFF_FFFF;
  let d = h.call(
    op::SEND_OBJECT_INFO,
    &[STORAGE_ID, root],
    Some(&object_info("Inbox", format::ASSOCIATION, 0)),
  );
  assert_eq!(d.code, resp::OK);
  let inbox = d.params[2];
  assert_eq!(h.disk.nodes.get("/Inbox"), Some(&Node::Dir));

  // Rename keeps the handle.
  let a = h.find(root, "a.mp3");
  let renamed = h.call(
    op::SET_OBJECT_PROP_VALUE,
    &[a, prop::OBJECT_FILE_NAME as u32],
    Some(&ptp_str("b.mp3")),
  );
  assert_eq!(renamed.code, resp::OK);
  assert_eq!(h.name_of(a), "b.mp3");

  // Move keeps the handle and the bytes.
  assert_eq!(
    h.call(op::MOVE_OBJECT, &[a, STORAGE_ID, inbox], None).code,
    resp::OK
  );
  assert_eq!(h.disk.file("/Inbox/b.mp3"), Some(&[1u8; 10][..]));
  assert_eq!(h.handles(inbox), [a]);

  // A folder cannot be moved into itself.
  let albums = h.find(root, "Albums");
  let one = h.find(albums, "One");
  assert_eq!(
    h.call(op::MOVE_OBJECT, &[albums, STORAGE_ID, one], None)
      .code,
    resp::INVALID_PARENT_OBJECT
  );

  // Deleting a folder removes everything inside it, and its handles.
  let intro = h.find(one, "01 Intro.mp3");
  assert_eq!(h.call(op::DELETE_OBJECT, &[albums], None).code, resp::OK);
  assert!(h.disk.nodes.keys().all(|p| !p.starts_with("/Albums")));
  assert_eq!(
    h.call(op::GET_OBJECT_INFO, &[intro], None).code,
    resp::INVALID_OBJECT_HANDLE
  );
  assert_eq!(h.handles(root), [inbox]);
}

#[test]
fn files_removed_behind_the_hosts_back_lose_their_handles() {
  let mut h = Host::new(music_disk());
  let a = h.find(0xFFFF_FFFF, "a.mp3");
  h.disk.nodes.remove("/a.mp3");
  h.state.forget_names();
  assert_eq!(
    h.call(op::GET_OBJECT, &[a], None).code,
    resp::INVALID_OBJECT_HANDLE
  );
  assert!(!h.handles(0xFFFF_FFFF).contains(&a));
}

#[test]
fn prop_list_for_folder_children_matches_handles() {
  let mut h = Host::new(music_disk());
  let albums = h.find(0xFFFF_FFFF, "Albums");
  let one = h.find(albums, "One");
  let d = h.call(
    op::GET_OBJECT_PROP_LIST,
    &[one, 0, prop::OBJECT_FILE_NAME as u32, 0, 1],
    None,
  );
  assert_eq!(d.code, resp::OK);
  let count = u32::from_le_bytes(d.data[..4].try_into().unwrap());
  assert_eq!(count, 2);
  let mut names = StdVec::new();
  let mut at = 4;
  for _ in 0..count {
    let handle = u32::from_le_bytes(d.data[at..at + 4].try_into().unwrap());
    let mut n = Name::new();
    at += 8 + parse_str(&d.data[at + 8..], &mut n).unwrap();
    names.push((handle, StdString::from(n.as_str())));
  }
  let mut expect: StdVec<_> = h
    .handles(one)
    .into_iter()
    .map(|x| (x, h.name_of(x)))
    .collect();
  names.sort();
  expect.sort();
  assert_eq!(names, expect);
}
