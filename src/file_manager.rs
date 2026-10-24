use core::str::FromStr;

use embassy_stm32::sdmmc;
use heapless::{String, Vec};
use microsd::SdCard;

pub const ACCEPTED_EXTENSIONS: &[&str] = &["mp3"];
pub type FileName = String<256>;
pub type FileList = Vec<FileName, 100>;

pub type FS<'a, 'b> = exfat_slim::asynchronous::file_system::FileSystem<SdCard<'a, 'b>, 512, 4>;

pub async fn list_files<'a, 'b>(
  fs: &mut FS<'a, 'b>,
) -> Result<FileList, exfat_slim::asynchronous::error::ExFatError<sdmmc::Error>> {
  let mut files = FileList::new();

  let mut dir = fs.read_dir("/").await?;
  let mut name_buf = [0u8; 765];

  while let Some(entry) = dir.next_entry(fs, &mut name_buf).await? {
    if entry.metadata.is_dir() {
      continue;
    }
    let name = entry.name;
    if !ACCEPTED_EXTENSIONS.iter().any(|ext| name.ends_with(ext)) {
      continue;
    }

    if let Ok(fname) = FileName::from_str(name) {
      if files.push(fname).is_err() {
        break;
      }
    }
  }

  Ok(files)
}


