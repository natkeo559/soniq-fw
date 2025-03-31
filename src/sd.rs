// Copyright (c) 2025 Nathan H. Keough
//
// This work is dual-licensed under MIT OR Apache 2.0 (or any later version).
// You may choose between one of them if you use this work.
//
// For further detail, please refer to the individual licenses located at the root of this crate.
use embedded_sdmmc::{
    Error, Mode, SdCard, SdCardError, TimeSource, Timestamp, VolumeIdx, VolumeManager,
};

#[allow(unused)]
pub fn create_wav_header(
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    data_len: u32,
) -> [u8; 44] {
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align = channels * (bits_per_sample / 8);
    let chunk_size = 36 + data_len;
    let mut header = [0u8; 44];

    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&chunk_size.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");

    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes()); // PCM header size
    header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM format
    header[22..24].copy_from_slice(&channels.to_le_bytes());
    header[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&bits_per_sample.to_le_bytes());

    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_len.to_le_bytes());

    header
}

// Dummy `TimeSource` for `embedded_sdmmc`
pub struct DummyTime;

impl TimeSource for DummyTime {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 55, // e.g. 2025
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub fn try_sd_init<S, D, T>(spi: S, delay: D, ts: T) -> Result<(), Error<SdCardError>>
where
    S: embedded_hal::spi::SpiDevice,
    D: embedded_hal::delay::DelayNs,
    T: TimeSource,
{
    let sdcard = SdCard::new(spi, delay);
    defmt::info!("Card size is {} bytes", sdcard.num_bytes()?);
    let mut volume_mgr = VolumeManager::new(sdcard, ts);
    let mut volume0 = volume_mgr.open_volume(VolumeIdx(0))?;
    let mut root_dir = volume0.open_root_dir()?;
    let mut my_file = root_dir.open_file_in_dir("hello.txt", Mode::ReadOnly)?;
    while !my_file.is_eof() {
        let mut buffer = [0u8; 32];
        let num_read = my_file.read(&mut buffer)?;
        unsafe {
            defmt::info!("{}", core::str::from_utf8_unchecked(&buffer[0..num_read]));
        }
    }
    Ok(())
}
