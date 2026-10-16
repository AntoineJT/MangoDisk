//! Shared native storage access for temperature and low-frequency health reads.
use super::{
    disk::ResourceVolume,
    disk_health::DiskHealth,
    disk_temperature::{ata_temperature, nvme_temperature, unsupported, DiskTemperatureSource},
};
use crate::PlatformResult;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub(super) fn read_temperature(
    volume: &ResourceVolume,
) -> PlatformResult<(f64, DiskTemperatureSource)> {
    #[cfg(target_os = "macos")]
    {
        macos::read(volume)
    }
    #[cfg(windows)]
    {
        windows::read(volume)
    }
}

pub(super) fn read_health(volume: &ResourceVolume) -> PlatformResult<DiskHealth> {
    #[cfg(target_os = "macos")]
    {
        macos::read_health(volume)
    }
    #[cfg(windows)]
    {
        windows::read_health(volume)
    }
}
