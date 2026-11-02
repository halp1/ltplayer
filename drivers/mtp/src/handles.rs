//! Object handle table.
//!
//! Handles are assigned as the host lists folders and stay valid for the
//! session. Each slot stores its parent handle and a hash of its name, so the
//! path is rebuilt by walking up the parents and matching names in each
//! folder. Handles are never reused within a session.

use crate::codes::format;
use crate::dataset::{format_for, name_hash};

/// Handle of the root folder. Not a real object.
pub const ROOT: u32 = 0;
/// Deepest folder nesting the responder will resolve.
pub const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
  Alive,
  /// Announced by SendObjectInfo, not yet written by SendObject.
  Pending,
  Dead,
}

#[derive(Debug, Clone, Copy)]
pub struct Slot {
  pub hash: u64,
  pub parent: u32,
  pub format: u16,
  status: Status,
  seen: bool,
}

impl Slot {
  pub fn is_dir(&self) -> bool {
    self.format == format::ASSOCIATION
  }
}

pub struct HandleTable<const N: usize> {
  slots: heapless::Vec<Slot, N>,
}

impl<const N: usize> Default for HandleTable<N> {
  fn default() -> Self {
    Self::new()
  }
}

fn index(h: u32) -> Option<usize> {
  (h as usize).checked_sub(1)
}

impl<const N: usize> HandleTable<N> {
  pub const fn new() -> Self {
    Self {
      slots: heapless::Vec::new(),
    }
  }

  pub fn clear(&mut self) {
    self.slots.clear();
  }

  /// Slots assigned this session, dead ones included. Handles are `1..=len`.
  pub fn len(&self) -> usize {
    self.slots.len()
  }

  pub fn is_empty(&self) -> bool {
    self.slots.is_empty()
  }

  fn find(&self, parent: u32, hash: u64, is_dir: bool) -> Option<usize> {
    self.slots.iter().position(|s| {
      s.status != Status::Dead && s.parent == parent && s.hash == hash && s.is_dir() == is_dir
    })
  }

  /// Handle for `name` in `parent`, reusing the existing one if present, and
  /// marks it seen. Returns `None` when the table is full.
  pub fn assign(&mut self, parent: u32, name: &str, is_dir: bool) -> Option<u32> {
    let hash = name_hash(name);
    if let Some(i) = self.find(parent, hash, is_dir) {
      self.slots[i].seen = true;
      return Some(i as u32 + 1);
    }
    self
      .slots
      .push(Slot {
        hash,
        parent,
        format: format_for(name, is_dir),
        status: Status::Alive,
        seen: true,
      })
      .ok()?;
    Some(self.slots.len() as u32)
  }

  /// Like [`HandleTable::assign`] for a file, but the object stays hidden
  /// until [`HandleTable::commit`].
  pub fn assign_pending(&mut self, parent: u32, name: &str) -> Option<u32> {
    let h = self.assign(parent, name, false)?;
    self.slots[h as usize - 1].status = Status::Pending;
    Some(h)
  }

  pub fn commit(&mut self, h: u32) {
    if let Some(slot) = self.slot_mut(h) {
      slot.status = Status::Alive;
    }
  }

  /// Drop every slot that never received its data.
  pub fn discard_pending(&mut self) {
    for slot in self.slots.iter_mut() {
      if slot.status == Status::Pending {
        slot.status = Status::Dead;
      }
    }
  }

  /// Visible slot for `h`.
  pub fn get(&self, h: u32) -> Option<&Slot> {
    self
      .slots
      .get(index(h)?)
      .filter(|s| s.status == Status::Alive)
  }

  fn get_any(&self, h: u32) -> Option<&Slot> {
    self
      .slots
      .get(index(h)?)
      .filter(|s| s.status != Status::Dead)
  }

  fn slot_mut(&mut self, h: u32) -> Option<&mut Slot> {
    self
      .slots
      .get_mut(index(h)?)
      .filter(|s| s.status != Status::Dead)
  }

  pub fn is_dir(&self, h: u32) -> bool {
    h == ROOT || self.get(h).is_some_and(Slot::is_dir)
  }

  /// Record a rename or move.
  pub fn relocate(&mut self, h: u32, parent: u32, name: &str) {
    if let Some(slot) = self.slot_mut(h) {
      slot.parent = parent;
      slot.hash = name_hash(name);
      slot.format = format_for(name, slot.is_dir());
    }
  }

  /// `h` and its ancestors up to (not including) the root, nearest first.
  pub fn chain(&self, h: u32, out: &mut heapless::Vec<u32, MAX_DEPTH>) -> bool {
    out.clear();
    let mut cur = h;
    while cur != ROOT {
      let Some(slot) = self.get_any(cur) else {
        return false;
      };
      if out.push(cur).is_err() {
        return false;
      }
      cur = slot.parent;
    }
    true
  }

  /// True if `h` is `ancestor` or lies inside it.
  pub fn is_within(&self, h: u32, ancestor: u32) -> bool {
    let mut cur = h;
    for _ in 0..=MAX_DEPTH {
      if cur == ancestor {
        return true;
      }
      match self.get_any(cur) {
        Some(slot) => cur = slot.parent,
        None => return false,
      }
    }
    false
  }

  /// Mark `h` and everything inside it as gone.
  pub fn kill_subtree(&mut self, h: u32) {
    for i in 0..self.slots.len() {
      let handle = i as u32 + 1;
      if self.slots[i].status != Status::Dead && self.is_within(handle, h) {
        self.slots[i].status = Status::Dead;
      }
    }
  }

  /// Start a listing of `parent`: children not re-assigned before
  /// [`HandleTable::sweep`] are treated as deleted.
  pub fn mark_unseen(&mut self, parent: u32) {
    for slot in self.slots.iter_mut() {
      if slot.parent == parent && slot.status == Status::Alive {
        slot.seen = false;
      }
    }
  }

  pub fn sweep(&mut self, parent: u32) {
    for i in 0..self.slots.len() {
      let s = self.slots[i];
      if s.parent == parent && s.status == Status::Alive && !s.seen {
        self.kill_subtree(i as u32 + 1);
      }
    }
  }

  /// Visible handles, optionally restricted to one parent and one format
  /// (`0` means any format).
  pub fn visible(&self, parent: Option<u32>, format: u16) -> impl Iterator<Item = u32> + '_ {
    self
      .slots
      .iter()
      .enumerate()
      .filter(move |(_, s)| {
        s.status == Status::Alive
          && parent.is_none_or(|p| s.parent == p)
          && (format == 0 || s.format == format)
      })
      .map(|(i, _)| i as u32 + 1)
  }
}
