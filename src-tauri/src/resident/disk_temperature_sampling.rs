//! Preserve the temperature cadence across panel demand changes without retaining native handles.
use mangodisk_platform::{
    system_resources::{disk::ResourceVolume, disk_temperature::DiskTemperature},
    PlatformResult,
};

type Reading = (u64, PlatformResult<DiskTemperature>);
const INTERVAL_MS: u64 = 30_000;

#[derive(Default)]
pub(super) struct TemperatureSampler {
    identity: Option<(String, String)>,
    cached: Option<Reading>,
}

impl TemperatureSampler {
    pub fn read_with(
        &mut self,
        volume: &ResourceVolume,
        now_ms: u64,
        read: impl FnOnce() -> Reading,
    ) -> Reading {
        let identity = (volume.id.clone(), volume.mount_point.clone());
        if self.identity.as_ref() == Some(&identity) {
            if let Some(cached) = &self.cached {
                if now_ms
                    .checked_sub(cached.0)
                    .is_some_and(|elapsed| elapsed < INTERVAL_MS)
                {
                    return cached.clone();
                }
            }
        }
        let reading = read();
        self.identity = Some(identity);
        self.cached = Some(reading.clone());
        reading
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mangodisk_platform::{PlatformError, PlatformErrorCode};

    fn volume() -> ResourceVolume {
        ResourceVolume {
            id: "disk".into(),
            name: "Disk".into(),
            mount_point: "/".into(),
            file_system: None,
            system: true,
        }
    }

    fn missing(now_ms: u64) -> Reading {
        (
            now_ms,
            Err(PlatformError::new(
                PlatformErrorCode::Unsupported,
                "unavailable",
            )),
        )
    }

    #[test]
    fn panel_reopens_reuse_the_observation_timestamp_until_thirty_seconds() {
        let mut sampler = TemperatureSampler::default();
        let disk = volume();
        let first = sampler.read_with(&disk, 1000, || missing(1010));
        for now in [1010, 2000, 30_000, 31_009] {
            let cached = sampler.read_with(&disk, now, || panic!("reopening must not query SMART"));
            assert_eq!(cached.0, first.0);
            assert_eq!(cached.1.unwrap_err().code(), PlatformErrorCode::Unsupported);
        }
        assert_eq!(
            sampler.read_with(&disk, 31_010, || missing(31_020)).0,
            31_020
        );
    }

    #[test]
    fn successful_observations_refresh_once_per_interval_without_extending_freshness() {
        let mut sampler = TemperatureSampler::default();
        let disk = volume();
        let calls = std::cell::Cell::new(0);
        for now in [1000, 1001, 10_000, 30_999, 31_000] {
            let reading = sampler.read_with(&disk, now, || {
                calls.set(calls.get() + 1);
                (
                    now,
                    Ok(DiskTemperature {
                        volume_id: disk.id.clone(),
                        volume_name: disk.name.clone(),
                        celsius: 34.0 + f64::from(calls.get()),
                        source: mangodisk_platform::system_resources::disk_temperature::DiskTemperatureSource::NvmeSmart,
                    }),
                )
            });
            assert_eq!(reading.0, if now < 31_000 { 1000 } else { 31_000 });
            assert_eq!(
                reading.1.unwrap().celsius,
                if now < 31_000 { 35.0 } else { 36.0 }
            );
        }
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn changed_disks_mounts_clock_rollback_and_disconnect_invalidate_temperature() {
        let mut sampler = TemperatureSampler::default();
        let mut disk = volume();
        let _ = sampler.read_with(&disk, 1000, || missing(1000));
        disk.id = "other".into();
        assert_eq!(sampler.read_with(&disk, 1001, || missing(1001)).0, 1001);
        disk.mount_point = "/other".into();
        assert_eq!(sampler.read_with(&disk, 1002, || missing(1002)).0, 1002);
        assert_eq!(sampler.read_with(&disk, 10, || missing(10)).0, 10);
        sampler.clear();
        assert_eq!(sampler.read_with(&disk, 11, || missing(11)).0, 11);
    }
}
