//! PTP USB container framing: a 12-byte header plus payload.

pub const HEADER_LEN: usize = 12;
pub const MAX_PARAMS: usize = 5;
pub const RESPONSE_MAX: usize = HEADER_LEN + 4 * MAX_PARAMS;

pub mod kind {
  pub const COMMAND: u16 = 1;
  pub const DATA: u16 = 2;
  pub const RESPONSE: u16 = 3;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContainerError {
  TooShort,
  BadLength,
  WrongType,
  TooManyParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
  pub length: u32,
  pub kind: u16,
  pub code: u16,
  pub transaction_id: u32,
}

impl Header {
  pub fn parse(b: &[u8]) -> Result<Self, ContainerError> {
    if b.len() < HEADER_LEN {
      return Err(ContainerError::TooShort);
    }
    Ok(Self {
      length: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
      kind: u16::from_le_bytes([b[4], b[5]]),
      code: u16::from_le_bytes([b[6], b[7]]),
      transaction_id: u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
    })
  }

  pub fn encode(&self) -> [u8; HEADER_LEN] {
    let mut out = [0u8; HEADER_LEN];
    out[0..4].copy_from_slice(&self.length.to_le_bytes());
    out[4..6].copy_from_slice(&self.kind.to_le_bytes());
    out[6..8].copy_from_slice(&self.code.to_le_bytes());
    out[8..12].copy_from_slice(&self.transaction_id.to_le_bytes());
    out
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
  pub code: u16,
  pub transaction_id: u32,
  params: [u32; MAX_PARAMS],
  num_params: u8,
}

impl Command {
  /// Parse a command container. Commands always fit in one packet.
  pub fn parse(packet: &[u8]) -> Result<Self, ContainerError> {
    let header = Header::parse(packet)?;
    if header.kind != kind::COMMAND {
      return Err(ContainerError::WrongType);
    }
    let len = header.length as usize;
    if len < HEADER_LEN || len > packet.len() || !(len - HEADER_LEN).is_multiple_of(4) {
      return Err(ContainerError::BadLength);
    }
    let n = (len - HEADER_LEN) / 4;
    if n > MAX_PARAMS {
      return Err(ContainerError::TooManyParams);
    }
    let mut params = [0u32; MAX_PARAMS];
    for (i, p) in params.iter_mut().take(n).enumerate() {
      let o = HEADER_LEN + 4 * i;
      *p = u32::from_le_bytes([packet[o], packet[o + 1], packet[o + 2], packet[o + 3]]);
    }
    Ok(Self {
      code: header.code,
      transaction_id: header.transaction_id,
      params,
      num_params: n as u8,
    })
  }

  /// Parameter `i`, or 0 if the host omitted it.
  pub fn param(&self, i: usize) -> u32 {
    self.params.get(i).copied().unwrap_or(0)
  }

  pub fn params(&self) -> &[u32] {
    &self.params[..self.num_params as usize]
  }
}

pub fn encode_response(
  code: u16,
  transaction_id: u32,
  params: &[u32],
  out: &mut [u8; RESPONSE_MAX],
) -> usize {
  let n = params.len().min(MAX_PARAMS);
  let len = HEADER_LEN + 4 * n;
  let header = Header {
    length: len as u32,
    kind: kind::RESPONSE,
    code,
    transaction_id,
  };
  out[..HEADER_LEN].copy_from_slice(&header.encode());
  for (i, p) in params.iter().take(n).enumerate() {
    let o = HEADER_LEN + 4 * i;
    out[o..o + 4].copy_from_slice(&p.to_le_bytes());
  }
  len
}
