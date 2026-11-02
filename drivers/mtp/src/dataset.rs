//! PTP dataset encoding: little-endian integers, arrays and UCS-2 strings.

use crate::codes::format;

/// Longest file or folder name, in UTF-8 bytes.
pub const MAX_NAME: usize = 255;
pub type Name = heapless::String<MAX_NAME>;

/// PTP strings hold at most 255 UTF-16 units including the terminator.
const MAX_STR_UNITS: usize = 254;

/// Byte sink for datasets. The responder runs each dataset through a
/// [`Counter`] first to learn its length, then through the USB sender.
#[allow(async_fn_in_trait)]
pub trait Sink {
  type Err;

  async fn put(&mut self, bytes: &[u8]) -> Result<(), Self::Err>;

  async fn u8(&mut self, v: u8) -> Result<(), Self::Err> {
    self.put(&[v]).await
  }

  async fn u16(&mut self, v: u16) -> Result<(), Self::Err> {
    self.put(&v.to_le_bytes()).await
  }

  async fn u32(&mut self, v: u32) -> Result<(), Self::Err> {
    self.put(&v.to_le_bytes()).await
  }

  async fn u64(&mut self, v: u64) -> Result<(), Self::Err> {
    self.put(&v.to_le_bytes()).await
  }

  async fn u16_array(&mut self, values: &[u16]) -> Result<(), Self::Err> {
    self.u32(values.len() as u32).await?;
    for &v in values {
      self.u16(v).await?;
    }
    Ok(())
  }

  async fn u32_array(&mut self, values: &[u32]) -> Result<(), Self::Err> {
    self.u32(values.len() as u32).await?;
    for &v in values {
      self.u32(v).await?;
    }
    Ok(())
  }

  /// PTP string: unit count including the terminator, UTF-16LE units, then
  /// a null unit. Strings that are too long are cut at a character boundary.
  async fn str(&mut self, s: &str) -> Result<(), Self::Err> {
    let units = str_units(s);
    if units == 0 {
      return self.u8(0).await;
    }
    self.u8(units as u8 + 1).await?;
    let mut written = 0;
    for c in s.chars() {
      let mut tmp = [0u16; 2];
      let encoded = c.encode_utf16(&mut tmp);
      if written + encoded.len() > units {
        break;
      }
      for &u in encoded.iter() {
        self.u16(u).await?;
      }
      written += encoded.len();
    }
    self.u16(0).await
  }
}

/// UTF-16 units of `s` that fit in a PTP string.
fn str_units(s: &str) -> usize {
  let mut units = 0;
  for c in s.chars() {
    let n = c.len_utf16();
    if units + n > MAX_STR_UNITS {
      break;
    }
    units += n;
  }
  units
}

/// Counts bytes without writing them.
#[derive(Debug, Default)]
pub struct Counter {
  pub bytes: u64,
}

impl Sink for Counter {
  type Err = core::convert::Infallible;

  async fn put(&mut self, bytes: &[u8]) -> Result<(), Self::Err> {
    self.bytes += bytes.len() as u64;
    Ok(())
  }
}

/// Decode a PTP string at the start of `b` into `out`. Returns the bytes
/// consumed, or `None` if the string is malformed or longer than [`MAX_NAME`].
pub fn parse_str(b: &[u8], out: &mut Name) -> Option<usize> {
  out.clear();
  let n = *b.first()? as usize;
  if n == 0 {
    return Some(1);
  }
  let end = 1 + 2 * n;
  let raw = b.get(1..end)?;
  let units = raw
    .as_chunks::<2>()
    .0
    .iter()
    .map(|&u| u16::from_le_bytes(u))
    .take_while(|&u| u != 0);
  for c in char::decode_utf16(units) {
    out.push(c.ok()?).ok()?;
  }
  Some(end)
}

/// True if `name` is usable as a single path component.
pub fn valid_name(name: &str) -> bool {
  !name.is_empty()
    && name != "."
    && name != ".."
    && !name.contains(['/', '\\', '\0'])
    && name.trim() == name
}

/// MTP object format for a file name.
pub fn format_for(name: &str, is_dir: bool) -> u16 {
  if is_dir {
    return format::ASSOCIATION;
  }
  let Some((_, ext)) = name.rsplit_once('.') else {
    return format::UNDEFINED;
  };
  let eq = |e: &str| ext.eq_ignore_ascii_case(e);
  if eq("mp3") {
    format::MP3
  } else if eq("wav") {
    format::WAV
  } else if eq("flac") {
    format::FLAC
  } else if eq("ogg") || eq("opus") {
    format::OGG
  } else if eq("aac") || eq("m4a") {
    format::AAC
  } else {
    format::UNDEFINED
  }
}

/// FNV-1a over the name bytes. 64 bits keeps sibling collisions negligible.
pub fn name_hash(name: &str) -> u64 {
  let mut h: u64 = 0xcbf2_9ce4_8422_2325;
  for &b in name.as_bytes() {
    h ^= b as u64;
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
  }
  h
}
