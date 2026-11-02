use heapless::{String, Vec};

use crate::codes::{STORAGE_ID, device_prop, dtype, format, op, prop, resp};
use crate::container::{self, Command, HEADER_LEN, Header, kind};
use crate::dataset::{Name, Sink, format_for, parse_str, valid_name};
use crate::handles::{HandleTable, MAX_DEPTH, ROOT};
use crate::storage::{EntryMeta, Storage, StorageError};
use crate::transport::Transport;

pub const MAX_PATH: usize = 512;
pub type PathBuf = String<MAX_PATH>;

/// Largest bulk packet the responder can buffer.
const PKT: usize = 512;
/// Bytes moved per storage call during file transfers.
const CHUNK: usize = 4096;
/// Recently resolved names kept to avoid re-listing folders.
const CACHE: usize = 64;
/// "All" or "root" in handle and storage parameters.
const ALL: u32 = 0xFFFF_FFFF;

const DIR: EntryMeta = EntryMeta {
  is_dir: true,
  size: 0,
};

#[derive(Debug, Clone, Copy)]
pub struct DeviceStrings {
  pub manufacturer: &'static str,
  pub model: &'static str,
  pub version: &'static str,
  pub serial: &'static str,
  pub friendly_name: &'static str,
  pub storage_label: &'static str,
}

struct CacheEntry {
  handle: u32,
  meta: EntryMeta,
  name: Name,
}

struct NameCache {
  entries: Vec<CacheEntry, CACHE>,
  next: usize,
}

impl NameCache {
  const fn new() -> Self {
    Self {
      entries: Vec::new(),
      next: 0,
    }
  }

  fn get(&self, h: u32) -> Option<&CacheEntry> {
    self.entries.iter().find(|e| e.handle == h)
  }

  fn insert(&mut self, h: u32, name: &str, meta: EntryMeta) {
    let mut n = Name::new();
    if n.push_str(name).is_err() {
      return;
    }
    let entry = CacheEntry {
      handle: h,
      meta,
      name: n,
    };
    if let Some(e) = self.entries.iter_mut().find(|e| e.handle == h) {
      *e = entry;
    } else if let Err(entry) = self.entries.push(entry) {
      self.entries[self.next] = entry;
      self.next = (self.next + 1) % CACHE;
    }
  }

  fn remove(&mut self, h: u32) {
    self.entries.retain(|e| e.handle != h);
    self.next = 0;
  }

  fn clear(&mut self) {
    self.entries.clear();
    self.next = 0;
  }
}

struct Pending {
  handle: u32,
  size: u64,
  path: PathBuf,
  name: Name,
}

struct Core<const N: usize> {
  session: u32,
  table: HandleTable<N>,
  cache: NameCache,
  pending: Option<Pending>,
  strings: DeviceStrings,
}

impl<const N: usize> Core<N> {
  fn close_session(&mut self) {
    self.session = 0;
    self.pending = None;
    self.table.clear();
    self.cache.clear();
  }
}

/// Responder state. Large (about `24 * N + 23_000` bytes), so it is meant to
/// live in a `static`.
pub struct State<const N: usize> {
  core: Core<N>,
  pkt: [u8; PKT],
  chunk: [u8; CHUNK],
  path: PathBuf,
  path2: PathBuf,
}

impl<const N: usize> State<N> {
  /// Drop cached names so every handle is resolved from storage again.
  #[cfg(test)]
  pub(crate) fn forget_names(&mut self) {
    self.core.cache.clear();
  }

  pub const fn new(strings: DeviceStrings) -> Self {
    Self {
      core: Core {
        session: 0,
        table: HandleTable::new(),
        cache: NameCache::new(),
        pending: None,
        strings,
      },
      pkt: [0; PKT],
      chunk: [0; CHUNK],
      path: String::new(),
      path2: String::new(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reply {
  code: u16,
  params: [u32; 3],
  n: usize,
}

impl Reply {
  fn code(code: u16) -> Self {
    Self {
      code,
      params: [0; 3],
      n: 0,
    }
  }

  fn with(code: u16, p: &[u32]) -> Self {
    let mut params = [0; 3];
    params[..p.len()].copy_from_slice(p);
    Self {
      code,
      params,
      n: p.len(),
    }
  }
}

type Handled<E> = Result<Option<Reply>, E>;

/// Return an MTP response code from a handler.
macro_rules! tri {
  ($e:expr) => {
    match $e {
      Ok(v) => v,
      Err(code) => return Ok(Some(Reply::code(code))),
    }
  };
}

fn storage_code(e: StorageError) -> u16 {
  match e {
    StorageError::NotFound => resp::INVALID_OBJECT_HANDLE,
    StorageError::Full => resp::STORE_FULL,
    StorageError::Exists | StorageError::NotEmpty => resp::ACCESS_DENIED,
    StorageError::Io => resp::GENERAL_ERROR,
  }
}

fn parent_param(p: u32) -> u32 {
  if p == ALL { ROOT } else { p }
}

fn storage_ok(id: u32) -> bool {
  id == 0 || id == ALL || id == STORAGE_ID
}

fn join(dir: &str, name: &str, out: &mut PathBuf) -> Result<(), u16> {
  out.clear();
  let ok = out.push_str(dir).is_ok()
    && (dir.ends_with('/') || out.push('/').is_ok())
    && out.push_str(name).is_ok();
  if ok { Ok(()) } else { Err(resp::GENERAL_ERROR) }
}

fn parent_dir(path: &str) -> &str {
  match path.rfind('/') {
    Some(0) | None => "/",
    Some(i) => &path[..i],
  }
}

fn prop_type(p: u16) -> u16 {
  match p {
    prop::STORAGE_ID | prop::PARENT_OBJECT => dtype::U32,
    prop::OBJECT_FORMAT | prop::PROTECTION_STATUS => dtype::U16,
    prop::OBJECT_SIZE => dtype::U64,
    _ => dtype::STR,
  }
}

/// A resolved object.
struct Obj {
  handle: u32,
  parent: u32,
  meta: EntryMeta,
  name: Name,
}

impl Obj {
  fn format(&self) -> u16 {
    format_for(&self.name, self.meta.is_dir)
  }
}

/// List `dir` until `target` is found, assigning handles to everything seen
/// and caching the entries from the target onward.
async fn lookup<S: Storage, const N: usize>(
  core: &mut Core<N>,
  storage: &mut S,
  dir: &str,
  parent: u32,
  target: u32,
) -> Result<(Name, EntryMeta), u16> {
  let dir = if dir.is_empty() { "/" } else { dir };
  storage.open_dir(dir).await.map_err(storage_code)?;
  let mut name = Name::new();
  let mut found = None;
  let mut budget = CACHE;
  while let Some(meta) = storage.next_entry(&mut name).await.map_err(storage_code)? {
    let Some(h) = core.table.assign(parent, &name, meta.is_dir) else {
      continue;
    };
    if h == target {
      found = Some((name.clone(), meta));
    }
    if found.is_some() {
      core.cache.insert(h, &name, meta);
      budget -= 1;
      if budget == 0 {
        break;
      }
    }
  }
  found.ok_or_else(|| {
    core.table.kill_subtree(target);
    core.cache.remove(target);
    resp::INVALID_OBJECT_HANDLE
  })
}

/// Rebuild the path of `h` into `path`.
async fn resolve<S: Storage, const N: usize>(
  core: &mut Core<N>,
  storage: &mut S,
  h: u32,
  path: &mut PathBuf,
) -> Result<Obj, u16> {
  path.clear();
  if h == ROOT {
    let _ = path.push('/');
    return Ok(Obj {
      handle: ROOT,
      parent: ROOT,
      meta: DIR,
      name: Name::new(),
    });
  }
  let mut chain = Vec::<u32, MAX_DEPTH>::new();
  let Some(slot) = core.table.get(h) else {
    return Err(resp::INVALID_OBJECT_HANDLE);
  };
  let obj_parent = slot.parent;
  if !core.table.chain(h, &mut chain) {
    return Err(resp::INVALID_OBJECT_HANDLE);
  }
  let mut parent = ROOT;
  let mut last = None;
  for &node in chain.iter().rev() {
    let (name, meta) = match core.cache.get(node) {
      Some(e) => (e.name.clone(), e.meta),
      None => lookup(core, storage, path.as_str(), parent, node).await?,
    };
    if path.push('/').is_err() || path.push_str(&name).is_err() {
      return Err(resp::GENERAL_ERROR);
    }
    last = Some((name, meta));
    parent = node;
  }
  let (name, meta) = last.ok_or(resp::INVALID_OBJECT_HANDLE)?;
  Ok(Obj {
    handle: h,
    parent: obj_parent,
    meta,
    name,
  })
}

/// Delete a folder and everything in it, deepest entries first.
async fn delete_tree<S: Storage>(storage: &mut S, path: &mut PathBuf) -> Result<(), StorageError> {
  let mut stack = Vec::<usize, MAX_DEPTH>::new();
  let _ = stack.push(path.len());
  let mut name = Name::new();
  while let Some(&len) = stack.last() {
    path.truncate(len);
    storage.open_dir(path).await?;
    match storage.next_entry(&mut name).await? {
      None => {
        storage.remove(path).await?;
        stack.pop();
      }
      Some(meta) => {
        if path.push('/').is_err() || path.push_str(&name).is_err() {
          return Err(StorageError::Io);
        }
        if meta.is_dir {
          stack.push(path.len()).map_err(|_| StorageError::Io)?;
        } else {
          storage.remove(path).await?;
        }
      }
    }
  }
  Ok(())
}

/// Outgoing data phase that frames bytes into packets. The payload length is
/// fixed up front: extra bytes are dropped and missing bytes are zero-padded
/// so the host always sees a well-formed transfer.
struct Tx<'a, T: Transport> {
  t: &'a mut T,
  buf: &'a mut [u8; PKT],
  fill: usize,
  remaining: u64,
}

impl<'a, T: Transport> Tx<'a, T> {
  fn start(t: &'a mut T, buf: &'a mut [u8; PKT], cmd: &Command, payload: u64) -> Self {
    let total = HEADER_LEN as u64 + payload;
    let header = Header {
      length: u32::try_from(total).unwrap_or(u32::MAX),
      kind: kind::DATA,
      code: cmd.code,
      transaction_id: cmd.transaction_id,
    };
    buf[..HEADER_LEN].copy_from_slice(&header.encode());
    Self {
      t,
      buf,
      fill: HEADER_LEN,
      remaining: payload,
    }
  }

  async fn put(&mut self, bytes: &[u8]) -> Result<(), T::Error> {
    let take = (bytes.len() as u64).min(self.remaining) as usize;
    let mut b = &bytes[..take];
    self.remaining -= take as u64;
    let mp = T::MAX_PACKET;
    while !b.is_empty() {
      let n = (mp - self.fill).min(b.len());
      self.buf[self.fill..self.fill + n].copy_from_slice(&b[..n]);
      self.fill += n;
      b = &b[n..];
      if self.fill == mp {
        self.t.write_packet(&self.buf[..mp]).await?;
        self.fill = 0;
      }
    }
    Ok(())
  }

  /// Pad, then send the final short packet or a ZLP.
  async fn end(mut self) -> Result<(), T::Error> {
    let zeros = [0u8; 64];
    while self.remaining > 0 {
      let n = self.remaining.min(zeros.len() as u64) as usize;
      self.put(&zeros[..n]).await?;
    }
    self.t.write_packet(&self.buf[..self.fill]).await
  }
}

/// Counting pass or sending pass of a dataset.
enum Out<'a, T: Transport> {
  Count(u64),
  Send(Tx<'a, T>),
}

impl<T: Transport> Sink for Out<'_, T> {
  type Err = T::Error;

  async fn put(&mut self, bytes: &[u8]) -> Result<(), T::Error> {
    match self {
      Out::Count(n) => {
        *n += bytes.len() as u64;
        Ok(())
      }
      Out::Send(tx) => tx.put(bytes).await,
    }
  }
}

/// Send a dataset produced by `f`. `f` runs twice: once to measure the
/// length for the header, once to send. It must emit the same bytes both
/// times.
async fn send_data<T: Transport>(
  t: &mut T,
  pkt: &mut [u8; PKT],
  cmd: &Command,
  mut f: impl AsyncFnMut(&mut Out<'_, T>) -> Result<(), T::Error>,
) -> Result<(), T::Error> {
  let mut out = Out::Count(0);
  f(&mut out).await?;
  let Out::Count(len) = out else { unreachable!() };
  let mut out = Out::Send(Tx::start(t, pkt, cmd, len));
  f(&mut out).await?;
  match out {
    Out::Send(tx) => tx.end().await,
    Out::Count(_) => unreachable!(),
  }
}

/// Incoming data phase.
struct Rx<'a, T: Transport> {
  t: &'a mut T,
  buf: &'a mut [u8; PKT],
  pos: usize,
  len: usize,
  /// Payload bytes still expected. `u64::MAX` means "until a short packet".
  remaining: u64,
  last: bool,
  /// The first packet was not a data container for this command.
  bad: bool,
  /// The host ended the transfer before the announced length.
  incomplete: bool,
}

impl<'a, T: Transport> Rx<'a, T> {
  async fn start(t: &'a mut T, buf: &'a mut [u8; PKT], cmd: &Command) -> Result<Self, T::Error> {
    let mp = T::MAX_PACKET;
    let n = t.read_packet(&mut buf[..mp]).await?;
    let mut rx = Self {
      t,
      buf,
      pos: 0,
      len: 0,
      remaining: 0,
      last: true,
      bad: false,
      incomplete: false,
    };
    match Header::parse(&rx.buf[..n]) {
      Ok(h) if h.kind == kind::DATA && h.code == cmd.code => {
        rx.remaining = if h.length == u32::MAX {
          u64::MAX
        } else {
          (h.length as u64).saturating_sub(HEADER_LEN as u64)
        };
        rx.pos = HEADER_LEN;
        rx.len = n;
        rx.last = n < mp;
      }
      _ => rx.bad = true,
    }
    Ok(rx)
  }

  /// Fill `out` as far as possible. Returns 0 at the end of the transfer.
  async fn read(&mut self, out: &mut [u8]) -> Result<usize, T::Error> {
    let mp = T::MAX_PACKET;
    let mut w = 0;
    while w < out.len() && self.remaining > 0 {
      if self.pos == self.len {
        if self.last {
          if self.remaining != u64::MAX {
            self.incomplete = true;
          }
          self.remaining = 0;
          break;
        }
        let n = self.t.read_packet(&mut self.buf[..mp]).await?;
        self.pos = 0;
        self.len = n;
        self.last = n < mp;
        continue;
      }
      let n = ((self.len - self.pos) as u64)
        .min(self.remaining)
        .min((out.len() - w) as u64) as usize;
      out[w..w + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
      self.pos += n;
      w += n;
      if self.remaining != u64::MAX {
        self.remaining -= n as u64;
      }
    }
    Ok(w)
  }

  async fn drain(&mut self) -> Result<(), T::Error> {
    let mut sink = [0u8; 64];
    while self.read(&mut sink).await? > 0 {}
    Ok(())
  }
}

async fn emit_device_info<K: Sink>(k: &mut K, s: &DeviceStrings) -> Result<(), K::Err> {
  k.u16(100).await?;
  k.u32(6).await?;
  k.u16(100).await?;
  k.str("microsoft.com: 1.0; ").await?;
  k.u16(0).await?;
  k.u16_array(op::SUPPORTED).await?;
  k.u16_array(&[]).await?;
  k.u16_array(&[device_prop::FRIENDLY_NAME]).await?;
  k.u16_array(&[]).await?;
  k.u16_array(format::SUPPORTED).await?;
  k.str(s.manufacturer).await?;
  k.str(s.model).await?;
  k.str(s.version).await?;
  k.str(s.serial).await
}

async fn emit_object_info<K: Sink>(k: &mut K, o: &Obj) -> Result<(), K::Err> {
  k.u32(STORAGE_ID).await?;
  k.u16(o.format()).await?;
  k.u16(0).await?;
  k.u32(u32::try_from(o.meta.size).unwrap_or(u32::MAX))
    .await?;
  k.u16(0).await?;
  for _ in 0..6 {
    k.u32(0).await?;
  }
  k.u32(o.parent).await?;
  k.u16(if o.meta.is_dir { 1 } else { 0 }).await?;
  k.u32(0).await?;
  k.u32(0).await?;
  k.str(&o.name).await?;
  k.str("").await?;
  k.str("").await?;
  k.str("").await
}

async fn emit_prop_value<K: Sink>(k: &mut K, p: u16, o: &Obj) -> Result<(), K::Err> {
  match p {
    prop::STORAGE_ID => k.u32(STORAGE_ID).await,
    prop::OBJECT_FORMAT => k.u16(o.format()).await,
    prop::PROTECTION_STATUS => k.u16(0).await,
    prop::OBJECT_SIZE => k.u64(o.meta.size).await,
    prop::PARENT_OBJECT => k.u32(o.parent).await,
    _ => k.str(&o.name).await,
  }
}

async fn emit_prop_elements<K: Sink>(k: &mut K, props: &[u16], o: &Obj) -> Result<(), K::Err> {
  for &p in props {
    k.u32(o.handle).await?;
    k.u16(p).await?;
    k.u16(prop_type(p)).await?;
    emit_prop_value(k, p, o).await?;
  }
  Ok(())
}

pub struct Responder<'s, T: Transport, S: Storage, const N: usize> {
  transport: T,
  storage: S,
  state: &'s mut State<N>,
}

impl<'s, T: Transport, S: Storage, const N: usize> Responder<'s, T, S, N> {
  pub fn new(transport: T, storage: S, state: &'s mut State<N>) -> Self {
    assert!(T::MAX_PACKET >= container::RESPONSE_MAX && T::MAX_PACKET <= PKT);
    Self {
      transport,
      storage,
      state,
    }
  }

  pub fn storage(&mut self) -> &mut S {
    &mut self.storage
  }

  pub fn transport(&mut self) -> &mut T {
    &mut self.transport
  }

  pub fn session_open(&self) -> bool {
    self.state.core.session != 0
  }

  /// Forget the session, as after a USB reset or detach.
  pub fn reset(&mut self) {
    self.state.core.close_session();
  }

  /// Serve commands until the transport fails.
  pub async fn run(&mut self) -> T::Error {
    loop {
      if let Err(e) = self.serve_one().await {
        return e;
      }
    }
  }

  /// Read and handle one command, including its data and response phases.
  pub async fn serve_one(&mut self) -> Result<(), T::Error> {
    let mp = T::MAX_PACKET;
    let n = self
      .transport
      .read_packet(&mut self.state.pkt[..mp])
      .await?;
    if n == 0 {
      // Trailing ZLP of a previous data phase.
      return Ok(());
    }
    let Ok(cmd) = Command::parse(&self.state.pkt[..n]) else {
      return Ok(());
    };
    self.transport.take_cancel();
    if let Some(reply) = self.dispatch(&cmd).await? {
      let mut out = [0u8; container::RESPONSE_MAX];
      let len = container::encode_response(
        reply.code,
        cmd.transaction_id,
        &reply.params[..reply.n],
        &mut out,
      );
      self.transport.write_packet(&out[..len]).await?;
    }
    Ok(())
  }

  async fn drain_data(&mut self, cmd: &Command) -> Result<(), T::Error> {
    if op::host_sends_data(cmd.code) {
      let mut rx = Rx::start(&mut self.transport, &mut self.state.pkt, cmd).await?;
      rx.drain().await?;
    }
    Ok(())
  }

  async fn dispatch(&mut self, cmd: &Command) -> Handled<T::Error> {
    let needs_session = !matches!(cmd.code, op::GET_DEVICE_INFO | op::OPEN_SESSION);
    if needs_session && self.state.core.session == 0 {
      self.drain_data(cmd).await?;
      return Ok(Some(Reply::code(resp::SESSION_NOT_OPEN)));
    }
    match cmd.code {
      op::GET_DEVICE_INFO => {
        let Self {
          transport, state, ..
        } = self;
        let strings = state.core.strings;
        send_data(
          transport,
          &mut state.pkt,
          cmd,
          async |k: &mut Out<'_, T>| emit_device_info(k, &strings).await,
        )
        .await?;
        Ok(Some(Reply::code(resp::OK)))
      }
      op::OPEN_SESSION => Ok(Some(self.open_session(cmd.param(0)))),
      op::CLOSE_SESSION => {
        self.state.core.close_session();
        Ok(Some(Reply::code(resp::OK)))
      }
      op::GET_STORAGE_IDS => {
        let Self {
          transport, state, ..
        } = self;
        send_data(
          transport,
          &mut state.pkt,
          cmd,
          async |k: &mut Out<'_, T>| k.u32_array(&[STORAGE_ID]).await,
        )
        .await?;
        Ok(Some(Reply::code(resp::OK)))
      }
      op::GET_STORAGE_INFO => self.storage_info(cmd).await,
      op::GET_NUM_OBJECTS => self.object_handles(cmd, true).await,
      op::GET_OBJECT_HANDLES => self.object_handles(cmd, false).await,
      op::GET_OBJECT_INFO => self.object_info(cmd).await,
      op::GET_OBJECT => self.get_object(cmd, false).await,
      op::GET_PARTIAL_OBJECT => self.get_object(cmd, true).await,
      op::DELETE_OBJECT => self.delete_object(cmd).await,
      op::SEND_OBJECT_INFO => self.send_object_info(cmd).await,
      op::SEND_OBJECT => self.send_object(cmd).await,
      op::MOVE_OBJECT => self.move_object(cmd).await,
      op::GET_DEVICE_PROP_DESC | op::GET_DEVICE_PROP_VALUE => self.device_prop(cmd).await,
      op::GET_OBJECT_PROPS_SUPPORTED => {
        let Self {
          transport, state, ..
        } = self;
        send_data(
          transport,
          &mut state.pkt,
          cmd,
          async |k: &mut Out<'_, T>| k.u16_array(prop::SUPPORTED).await,
        )
        .await?;
        Ok(Some(Reply::code(resp::OK)))
      }
      op::GET_OBJECT_PROP_DESC => self.object_prop_desc(cmd).await,
      op::GET_OBJECT_PROP_VALUE => self.object_prop_value(cmd).await,
      op::SET_OBJECT_PROP_VALUE => self.set_object_prop_value(cmd).await,
      op::GET_OBJECT_PROP_LIST => self.object_prop_list(cmd).await,
      _ => {
        self.drain_data(cmd).await?;
        Ok(Some(Reply::code(resp::OPERATION_NOT_SUPPORTED)))
      }
    }
  }

  fn open_session(&mut self, id: u32) -> Reply {
    let core = &mut self.state.core;
    if id == 0 {
      return Reply::code(resp::INVALID_PARAMETER);
    }
    if core.session != 0 {
      return Reply::with(resp::SESSION_ALREADY_OPEN, &[core.session]);
    }
    core.close_session();
    core.session = id;
    Reply::code(resp::OK)
  }

  async fn storage_info(&mut self, cmd: &Command) -> Handled<T::Error> {
    if cmd.param(0) != STORAGE_ID {
      return Ok(Some(Reply::code(resp::INVALID_STORAGE_ID)));
    }
    let (total, free) = self.storage.capacity();
    let Self {
      transport, state, ..
    } = self;
    let label = state.core.strings.storage_label;
    send_data(
      transport,
      &mut state.pkt,
      cmd,
      async |k: &mut Out<'_, T>| {
        k.u16(4).await?; // removable RAM
        k.u16(2).await?; // generic hierarchical
        k.u16(0).await?; // read-write
        k.u64(total).await?;
        k.u64(free).await?;
        k.u32(ALL).await?;
        k.str(label).await?;
        k.str(label).await
      },
    )
    .await?;
    Ok(Some(Reply::code(resp::OK)))
  }

  /// Re-list `parent` so new entries get handles and removed ones die.
  async fn refresh_dir(&mut self, parent: u32) -> Result<(), u16> {
    let State { core, path, .. } = &mut *self.state;
    resolve(core, &mut self.storage, parent, path).await?;
    core.table.mark_unseen(parent);
    self.storage.open_dir(path).await.map_err(storage_code)?;
    let mut name = Name::new();
    let mut cached = 0;
    while let Some(meta) = self
      .storage
      .next_entry(&mut name)
      .await
      .map_err(storage_code)?
    {
      let Some(h) = core.table.assign(parent, &name, meta.is_dir) else {
        #[cfg(feature = "defmt")]
        defmt::warn!("mtp: handle table full");
        continue;
      };
      if cached < CACHE {
        core.cache.insert(h, &name, meta);
        cached += 1;
      }
    }
    core.table.sweep(parent);
    Ok(())
  }

  async fn object_handles(&mut self, cmd: &Command, count_only: bool) -> Handled<T::Error> {
    if !storage_ok(cmd.param(0)) {
      return Ok(Some(Reply::code(resp::INVALID_STORAGE_ID)));
    }
    let fmt = cmd.param(1) as u16;
    // Parent 0 means "all objects". Only the root level is listed for it.
    let parent = parent_param(cmd.param(2));
    if !self.state.core.table.is_dir(parent) {
      return Ok(Some(Reply::code(resp::INVALID_PARENT_OBJECT)));
    }
    tri!(self.refresh_dir(parent).await);
    let Self {
      transport, state, ..
    } = self;
    let State { core, pkt, .. } = &mut **state;
    let count = core.table.visible(Some(parent), fmt).count() as u32;
    if count_only {
      return Ok(Some(Reply::with(resp::OK, &[count])));
    }
    let table = &core.table;
    send_data(transport, pkt, cmd, async |k: &mut Out<'_, T>| {
      k.u32(count).await?;
      for h in table.visible(Some(parent), fmt) {
        k.u32(h).await?;
      }
      Ok(())
    })
    .await?;
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn object_info(&mut self, cmd: &Command) -> Handled<T::Error> {
    let State {
      core, pkt, path, ..
    } = &mut *self.state;
    let obj = tri!(resolve(core, &mut self.storage, cmd.param(0), path).await);
    if obj.handle == ROOT {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_HANDLE)));
    }
    send_data(&mut self.transport, pkt, cmd, async |k: &mut Out<'_, T>| {
      emit_object_info(k, &obj).await
    })
    .await?;
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn get_object(&mut self, cmd: &Command, partial: bool) -> Handled<T::Error> {
    let State {
      core,
      pkt,
      chunk,
      path,
      ..
    } = &mut *self.state;
    let obj = tri!(resolve(core, &mut self.storage, cmd.param(0), path).await);
    if obj.meta.is_dir {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_HANDLE)));
    }
    let size = obj.meta.size;
    let (start, len) = if partial {
      let off = cmd.param(1) as u64;
      if off > size {
        return Ok(Some(Reply::code(resp::INVALID_PARAMETER)));
      }
      (off, (cmd.param(2) as u64).min(size - off))
    } else {
      (0, size)
    };
    let mut tx = Tx::start(&mut self.transport, pkt, cmd, len);
    let end = start + len;
    let mut pos = start;
    while pos < end {
      if tx.t.take_cancel() {
        return Ok(None);
      }
      let want = (end - pos).min(CHUNK as u64) as usize;
      match self.storage.read(path, pos, &mut chunk[..want]).await {
        Ok(n) if n > 0 => {
          tx.put(&chunk[..n]).await?;
          pos += n as u64;
        }
        _ => break,
      }
    }
    tx.end().await?;
    if pos < end {
      return Ok(Some(Reply::code(resp::INCOMPLETE_TRANSFER)));
    }
    if partial {
      Ok(Some(Reply::with(resp::OK, &[len as u32])))
    } else {
      Ok(Some(Reply::code(resp::OK)))
    }
  }

  async fn delete_object(&mut self, cmd: &Command) -> Handled<T::Error> {
    let h = cmd.param(0);
    if h == ALL || h == ROOT {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_HANDLE)));
    }
    let State { core, path, .. } = &mut *self.state;
    let obj = tri!(resolve(core, &mut self.storage, h, path).await);
    let result = if obj.meta.is_dir {
      delete_tree(&mut self.storage, path).await
    } else {
      self.storage.remove(path).await
    };
    if let Err(e) = result {
      // A partly deleted folder must be re-listed to learn what is left.
      core.cache.clear();
      return Ok(Some(Reply::code(storage_code(e))));
    }
    core.table.kill_subtree(h);
    core.cache.remove(h);
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn send_object_info(&mut self, cmd: &Command) -> Handled<T::Error> {
    let State {
      core,
      pkt,
      chunk,
      path,
      ..
    } = &mut *self.state;
    core.pending = None;
    core.table.discard_pending();

    let mut rx = Rx::start(&mut self.transport, pkt, cmd).await?;
    let n = rx.read(&mut chunk[..]).await?;
    rx.drain().await?;
    if rx.bad || rx.incomplete {
      return Ok(Some(Reply::code(resp::INVALID_DATASET)));
    }
    if !storage_ok(cmd.param(0)) {
      return Ok(Some(Reply::code(resp::INVALID_STORAGE_ID)));
    }
    let parent = parent_param(cmd.param(1));
    if !core.table.is_dir(parent) {
      return Ok(Some(Reply::code(resp::INVALID_PARENT_OBJECT)));
    }

    const NAME_AT: usize = 52;
    let info = &chunk[..n];
    let mut name = Name::new();
    if info.len() <= NAME_AT || parse_str(&info[NAME_AT..], &mut name).is_none() {
      return Ok(Some(Reply::code(resp::INVALID_DATASET)));
    }
    if !valid_name(&name) {
      return Ok(Some(Reply::code(resp::INVALID_DATASET)));
    }
    let fmt = u16::from_le_bytes([info[4], info[5]]);
    let size = u32::from_le_bytes([info[8], info[9], info[10], info[11]]);

    tri!(resolve(core, &mut self.storage, parent, path).await);
    let mut full = PathBuf::new();
    tri!(join(path, &name, &mut full));
    let parent_out = if parent == ROOT { ALL } else { parent };

    if fmt == format::ASSOCIATION {
      tri!(self.storage.mkdir(&full).await.map_err(storage_code));
      let Some(h) = core.table.assign(parent, &name, true) else {
        return Ok(Some(Reply::code(resp::STORE_FULL)));
      };
      core.cache.insert(h, &name, DIR);
      return Ok(Some(Reply::with(resp::OK, &[STORAGE_ID, parent_out, h])));
    }

    if size == u32::MAX {
      return Ok(Some(Reply::code(resp::OBJECT_TOO_LARGE)));
    }
    let (_, free) = self.storage.capacity();
    if size as u64 > free {
      return Ok(Some(Reply::code(resp::STORE_FULL)));
    }
    let Some(h) = core.table.assign_pending(parent, &name) else {
      return Ok(Some(Reply::code(resp::STORE_FULL)));
    };
    core.cache.remove(h);
    core.pending = Some(Pending {
      handle: h,
      size: size as u64,
      path: full,
      name,
    });
    Ok(Some(Reply::with(resp::OK, &[STORAGE_ID, parent_out, h])))
  }

  async fn send_object(&mut self, cmd: &Command) -> Handled<T::Error> {
    let State {
      core, pkt, chunk, ..
    } = &mut *self.state;
    let Some(p) = core.pending.take() else {
      let mut rx = Rx::start(&mut self.transport, pkt, cmd).await?;
      rx.drain().await?;
      return Ok(Some(Reply::code(resp::NO_VALID_OBJECT_INFO)));
    };
    let mut rx = Rx::start(&mut self.transport, pkt, cmd).await?;
    if rx.bad {
      core.table.discard_pending();
      return Ok(Some(Reply::code(resp::INVALID_DATASET)));
    }
    if let Err(e) = self.storage.create(&p.path, p.size).await {
      core.table.discard_pending();
      rx.drain().await?;
      return Ok(Some(Reply::code(storage_code(e))));
    }

    let mut written = 0u64;
    let failure = loop {
      if rx.t.take_cancel() {
        let _ = self.storage.abort().await;
        core.table.discard_pending();
        return Ok(None);
      }
      let n = match rx.read(&mut chunk[..]).await {
        Ok(n) => n,
        Err(e) => {
          let _ = self.storage.abort().await;
          core.table.discard_pending();
          return Err(e);
        }
      };
      if n == 0 {
        break None;
      }
      if let Err(e) = self.storage.write(&chunk[..n]).await {
        rx.drain().await?;
        break Some(storage_code(e));
      }
      written += n as u64;
    };
    let mut failure =
      failure.or((rx.incomplete || written != p.size).then_some(resp::INCOMPLETE_TRANSFER));
    if failure.is_none() {
      failure = self.storage.finish().await.err().map(storage_code);
    }
    if let Some(code) = failure {
      let _ = self.storage.abort().await;
      core.table.discard_pending();
      return Ok(Some(Reply::code(code)));
    }
    core.table.commit(p.handle);
    core.cache.insert(
      p.handle,
      &p.name,
      EntryMeta {
        is_dir: false,
        size: written,
      },
    );
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn move_object(&mut self, cmd: &Command) -> Handled<T::Error> {
    let h = cmd.param(0);
    let new_parent = parent_param(cmd.param(2));
    if !storage_ok(cmd.param(1)) {
      return Ok(Some(Reply::code(resp::INVALID_STORAGE_ID)));
    }
    let State {
      core, path, path2, ..
    } = &mut *self.state;
    if h == ROOT || h == ALL {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_HANDLE)));
    }
    if !core.table.is_dir(new_parent) || core.table.is_within(new_parent, h) {
      return Ok(Some(Reply::code(resp::INVALID_PARENT_OBJECT)));
    }
    let obj = tri!(resolve(core, &mut self.storage, h, path).await);
    tri!(resolve(core, &mut self.storage, new_parent, path2).await);
    let mut dest = PathBuf::new();
    tri!(join(path2, &obj.name, &mut dest));
    tri!(self.storage.rename(path, &dest).await.map_err(storage_code));
    core.table.relocate(h, new_parent, &obj.name);
    core.cache.insert(h, &obj.name, obj.meta);
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn device_prop(&mut self, cmd: &Command) -> Handled<T::Error> {
    if cmd.param(0) as u16 != device_prop::FRIENDLY_NAME {
      return Ok(Some(Reply::code(resp::DEVICE_PROP_NOT_SUPPORTED)));
    }
    let Self {
      transport, state, ..
    } = self;
    let name = state.core.strings.friendly_name;
    let desc = cmd.code == op::GET_DEVICE_PROP_DESC;
    send_data(
      transport,
      &mut state.pkt,
      cmd,
      async |k: &mut Out<'_, T>| {
        if desc {
          k.u16(device_prop::FRIENDLY_NAME).await?;
          k.u16(dtype::STR).await?;
          k.u8(0).await?;
          k.str(name).await?;
          k.str(name).await?;
          k.u8(0).await
        } else {
          k.str(name).await
        }
      },
    )
    .await?;
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn object_prop_desc(&mut self, cmd: &Command) -> Handled<T::Error> {
    let p = cmd.param(0) as u16;
    if !prop::SUPPORTED.contains(&p) {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_PROP_CODE)));
    }
    let Self {
      transport, state, ..
    } = self;
    send_data(
      transport,
      &mut state.pkt,
      cmd,
      async |k: &mut Out<'_, T>| {
        let t = prop_type(p);
        k.u16(p).await?;
        k.u16(t).await?;
        k.u8(u8::from(p == prop::OBJECT_FILE_NAME)).await?;
        match t {
          dtype::U16 => k.u16(0).await?,
          dtype::U32 => k.u32(0).await?,
          dtype::U64 => k.u64(0).await?,
          _ => k.u8(0).await?,
        }
        k.u32(0).await?;
        k.u8(0).await
      },
    )
    .await?;
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn object_prop_value(&mut self, cmd: &Command) -> Handled<T::Error> {
    let p = cmd.param(1) as u16;
    if !prop::SUPPORTED.contains(&p) {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_PROP_CODE)));
    }
    let State {
      core, pkt, path, ..
    } = &mut *self.state;
    let obj = tri!(resolve(core, &mut self.storage, cmd.param(0), path).await);
    if obj.handle == ROOT {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_HANDLE)));
    }
    send_data(&mut self.transport, pkt, cmd, async |k: &mut Out<'_, T>| {
      emit_prop_value(k, p, &obj).await
    })
    .await?;
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn set_object_prop_value(&mut self, cmd: &Command) -> Handled<T::Error> {
    let State {
      core,
      pkt,
      chunk,
      path,
      path2,
    } = &mut *self.state;
    let mut rx = Rx::start(&mut self.transport, pkt, cmd).await?;
    let n = rx.read(&mut chunk[..]).await?;
    rx.drain().await?;
    if rx.bad || rx.incomplete {
      return Ok(Some(Reply::code(resp::INVALID_DATASET)));
    }
    let p = cmd.param(1) as u16;
    if p != prop::OBJECT_FILE_NAME {
      let code = if prop::SUPPORTED.contains(&p) {
        resp::ACCESS_DENIED
      } else {
        resp::INVALID_OBJECT_PROP_CODE
      };
      return Ok(Some(Reply::code(code)));
    }
    let mut name = Name::new();
    if parse_str(&chunk[..n], &mut name).is_none() || !valid_name(&name) {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_PROP_VALUE)));
    }
    let h = cmd.param(0);
    let obj = tri!(resolve(core, &mut self.storage, h, path).await);
    if obj.handle == ROOT {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_HANDLE)));
    }
    tri!(join(parent_dir(path), &name, path2));
    tri!(self.storage.rename(path, path2).await.map_err(storage_code));
    core.table.relocate(h, obj.parent, &name);
    core.cache.insert(h, &name, obj.meta);
    Ok(Some(Reply::code(resp::OK)))
  }

  async fn object_prop_list(&mut self, cmd: &Command) -> Handled<T::Error> {
    let h = cmd.param(0);
    let fmt = cmd.param(1) as u16;
    let p = cmd.param(2);
    if p == 0 {
      return Ok(Some(Reply::code(resp::SPEC_BY_GROUP_UNSUPPORTED)));
    }
    let one = [p as u16];
    let props: &[u16] = if p == ALL {
      prop::SUPPORTED
    } else if prop::SUPPORTED.contains(&(p as u16)) {
      &one
    } else {
      return Ok(Some(Reply::code(resp::INVALID_OBJECT_PROP_CODE)));
    };

    match cmd.param(4) {
      0 => {
        let State {
          core, pkt, path, ..
        } = &mut *self.state;
        let obj = tri!(resolve(core, &mut self.storage, h, path).await);
        if obj.handle == ROOT {
          return Ok(Some(Reply::code(resp::INVALID_OBJECT_HANDLE)));
        }
        send_data(&mut self.transport, pkt, cmd, async |k: &mut Out<'_, T>| {
          k.u32(props.len() as u32).await?;
          emit_prop_elements(k, props, &obj).await
        })
        .await?;
      }
      1 => {
        let parent = parent_param(h);
        if !self.state.core.table.is_dir(parent) {
          return Ok(Some(Reply::code(resp::INVALID_PARENT_OBJECT)));
        }
        tri!(self.refresh_dir(parent).await);
        let Self {
          transport,
          storage,
          state,
        } = self;
        let State {
          core, pkt, path, ..
        } = &mut **state;
        let path = path.as_str();
        let mut elements = 0u32;
        send_data(transport, pkt, cmd, async |k: &mut Out<'_, T>| {
          let counting = matches!(k, Out::Count(_));
          k.u32(elements).await?;
          if storage.open_dir(path).await.is_err() {
            return Ok(());
          }
          let mut obj = Obj {
            handle: 0,
            parent,
            meta: DIR,
            name: Name::new(),
          };
          while let Ok(Some(meta)) = storage.next_entry(&mut obj.name).await {
            let Some(child) = core.table.assign(parent, &obj.name, meta.is_dir) else {
              continue;
            };
            obj.handle = child;
            obj.meta = meta;
            if fmt != 0 && obj.format() != fmt {
              continue;
            }
            if counting {
              elements += props.len() as u32;
            }
            emit_prop_elements(k, props, &obj).await?;
          }
          Ok(())
        })
        .await?;
      }
      _ => return Ok(Some(Reply::code(resp::SPEC_BY_DEPTH_UNSUPPORTED))),
    }
    Ok(Some(Reply::code(resp::OK)))
  }
}
