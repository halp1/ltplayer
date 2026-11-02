/// Bulk packet transport between the responder and the host.
#[allow(async_fn_in_trait)]
pub trait Transport {
  type Error;

  /// Max packet size of the bulk endpoints (64 for USB full speed).
  const MAX_PACKET: usize;

  /// Read one packet from the bulk OUT endpoint. A return shorter than
  /// `MAX_PACKET` (including 0) ends a transfer.
  async fn read_packet(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;

  /// Write one packet to the bulk IN endpoint. An empty slice sends a ZLP.
  async fn write_packet(&mut self, data: &[u8]) -> Result<(), Self::Error>;

  /// True once if the host issued a class Cancel request since the last call.
  fn take_cancel(&mut self) -> bool {
    false
  }
}
