//! Builds a private committed image without SQLite accessing the source files.
use crate::StorageError;
use std::{fs::File, io::Read, path::Path};

fn word(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap())
}

fn checksum(bytes: &[u8], mut sum: [u32; 2], big: bool) -> [u32; 2] {
    for pair in bytes.chunks_exact(8) {
        let read = |b: &[u8]| {
            if big {
                u32::from_be_bytes(b.try_into().unwrap())
            } else {
                u32::from_le_bytes(b.try_into().unwrap())
            }
        };
        sum[0] = sum[0].wrapping_add(read(&pair[..4])).wrapping_add(sum[1]);
        sum[1] = sum[1].wrapping_add(read(&pair[4..])).wrapping_add(sum[0]);
    }
    sum
}

fn read(path: &Path, limit: usize) -> Result<Vec<u8>, StorageError> {
    let file = File::open(path)?;
    let length = usize::try_from(file.metadata()?.len()).map_err(|_| StorageError::Full)?;
    if length > limit {
        return Err(StorageError::Full);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| StorageError::Full)?;
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(StorageError::Full);
    }
    Ok(bytes)
}

pub(crate) fn image(path: &Path, limit: usize) -> Result<Vec<u8>, StorageError> {
    let mut image = read(path, limit)?;
    if image.len() < 100 {
        return Err(StorageError::Corrupt);
    }
    let page = u16::from_be_bytes([image[16], image[17]]);
    let page = if page == 1 { 65536 } else { usize::from(page) };
    if !(512..=65536).contains(&page) || !page.is_power_of_two() {
        return Err(StorageError::Corrupt);
    }
    let mut wal_path = path.as_os_str().to_os_string();
    wal_path.push("-wal");
    let wal = match read(Path::new(&wal_path), limit) {
        Ok(wal) => wal,
        Err(StorageError::CantOpen) => Vec::new(),
        Err(error) => return Err(error),
    };
    if !wal.is_empty() {
        overlay(&mut image, &wal, page, limit)?;
    }
    // Deserialized databases use rollback mode; these bytes belong only to the copy.
    image[18] = 1;
    image[19] = 1;
    Ok(image)
}

fn overlay(image: &mut Vec<u8>, wal: &[u8], page: usize, limit: usize) -> Result<(), StorageError> {
    if wal.len() < 32 {
        return Ok(());
    }
    let magic = word(wal);
    if ![0x377f0682, 0x377f0683].contains(&magic)
        || word(&wal[4..]) != 3_007_000
        || word(&wal[8..]) as usize != page
    {
        return Err(StorageError::Corrupt);
    }
    let big = magic == 0x377f0683;
    let mut sum = checksum(&wal[..24], [0, 0], big);
    if sum != [word(&wal[24..]), word(&wal[28..])] {
        return Err(StorageError::Corrupt);
    }
    let mut frames = Vec::new();
    let mut committed = None;
    for frame in wal[32..].chunks_exact(24 + page) {
        if frame[8..16] != wal[16..24] {
            break;
        }
        let next = checksum(&frame[24..], checksum(&frame[..8], sum, big), big);
        if next != [word(&frame[16..]), word(&frame[20..])] {
            break;
        }
        let number = word(frame) as usize;
        if number == 0 {
            return Err(StorageError::Corrupt);
        }
        frames.push((number, &frame[24..]));
        sum = next;
        let pages = word(&frame[4..]) as usize;
        if pages != 0 {
            committed = Some((frames.len(), pages));
        }
    }
    if let Some((count, pages)) = committed {
        let size = pages
            .checked_mul(page)
            .filter(|n| *n <= limit)
            .ok_or(StorageError::Full)?;
        if size > image.len() {
            image
                .try_reserve_exact(size - image.len())
                .map_err(|_| StorageError::Full)?;
        }
        image.resize(size, 0);
        for (number, bytes) in frames.into_iter().take(count) {
            if number <= pages {
                image[(number - 1) * page..number * page].copy_from_slice(bytes);
            }
        }
    }
    Ok(())
}
