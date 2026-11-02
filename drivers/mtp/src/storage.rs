use crate::dataset::Name;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum StorageError {
  NotFound,
  Exists,
  NotEmpty,
  Full,
  Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryMeta {
  pub is_dir: bool,
  pub size: u64,
}

/// Filesystem the responder serves. Paths are absolute, `/` separated, and
/// `/` is the root folder.
#[allow(async_fn_in_trait)]
pub trait Storage {
  /// Start listing `dir`. Any other call on the storage ends the listing.
  async fn open_dir(&mut self, dir: &str) -> Result<(), StorageError>;

  /// Next entry of the open listing, with its name written to `name`.
  async fn next_entry(&mut self, name: &mut Name) -> Result<Option<EntryMeta>, StorageError>;

  /// Read file bytes at `offset`. Returns 0 at end of file.
  async fn read(&mut self, path: &str, offset: u64, buf: &mut [u8]) -> Result<usize, StorageError>;

  /// Create or truncate a file and keep it open for [`Storage::write`].
  /// Fails with `Exists` if `path` is a folder.
  async fn create(&mut self, path: &str, size: u64) -> Result<(), StorageError>;

  /// Append to the file opened by [`Storage::create`].
  async fn write(&mut self, data: &[u8]) -> Result<(), StorageError>;

  /// Close the file opened by [`Storage::create`].
  async fn finish(&mut self) -> Result<(), StorageError>;

  /// Delete the file opened by [`Storage::create`] if it was not finished.
  async fn abort(&mut self) -> Result<(), StorageError>;

  /// Remove a file or an empty folder.
  async fn remove(&mut self, path: &str) -> Result<(), StorageError>;

  /// Rename or move a file or folder. Fails with `Exists` if `to` exists.
  async fn rename(&mut self, from: &str, to: &str) -> Result<(), StorageError>;

  async fn mkdir(&mut self, path: &str) -> Result<(), StorageError>;

  /// `(total_bytes, free_bytes)`
  fn capacity(&self) -> (u64, u64);
}

impl<S: Storage> Storage for &mut S {
  async fn open_dir(&mut self, dir: &str) -> Result<(), StorageError> {
    (**self).open_dir(dir).await
  }

  async fn next_entry(&mut self, name: &mut Name) -> Result<Option<EntryMeta>, StorageError> {
    (**self).next_entry(name).await
  }

  async fn read(&mut self, path: &str, offset: u64, buf: &mut [u8]) -> Result<usize, StorageError> {
    (**self).read(path, offset, buf).await
  }

  async fn create(&mut self, path: &str, size: u64) -> Result<(), StorageError> {
    (**self).create(path, size).await
  }

  async fn write(&mut self, data: &[u8]) -> Result<(), StorageError> {
    (**self).write(data).await
  }

  async fn finish(&mut self) -> Result<(), StorageError> {
    (**self).finish().await
  }

  async fn abort(&mut self) -> Result<(), StorageError> {
    (**self).abort().await
  }

  async fn remove(&mut self, path: &str) -> Result<(), StorageError> {
    (**self).remove(path).await
  }

  async fn rename(&mut self, from: &str, to: &str) -> Result<(), StorageError> {
    (**self).rename(from, to).await
  }

  async fn mkdir(&mut self, path: &str) -> Result<(), StorageError> {
    (**self).mkdir(path).await
  }

  fn capacity(&self) -> (u64, u64) {
    (**self).capacity()
  }
}
