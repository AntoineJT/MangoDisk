use super::metrics::MetricStatus;
use super::readings::ResourceCache;
use mangodisk_platform::system_resources::{
    disk::{ResourceVolume, VolumeCapacity},
    disk_health::{DiskHealth, DiskHealthStatus, DISK_HEALTH_FRESHNESS_MS},
    disk_temperature::DiskTemperatureSource,
};

fn volume(id: &str) -> ResourceVolume {
    ResourceVolume {
        file_system: None,
        id: id.into(),
        name: "Disk".into(),
        system: true,
        mount_point: "/".into(),
    }
}
fn health(id: &str) -> DiskHealth {
    DiskHealth {
        volume_id: id.into(),
        volume_name: "Disk".into(),
        status: DiskHealthStatus::Healthy,
        source: DiskTemperatureSource::NvmeSmart,
        total_read_bytes: None,
        power_cycles: None,
        available_spare: None,
        critical_warning: None,
        percentage_used: Some(0),
        total_written_bytes: Some("123456789012345678901234".into()),
        power_on_hours: Some(0),
        media_errors: Some("0".into()),
        temperature_celsius: None,
        ata: None,
        nvme: None,
    }
}
#[test]
fn health_has_independent_freshness_and_rejects_other_devices_or_failed_observations() {
    let mut cache = ResourceCache::default();
    let capacity = VolumeCapacity {
        total_bytes: 100,
        available_bytes: 50,
    };
    cache.disk(volume("one"), capacity, 1000);
    cache.disk_health(Ok(health("one")), 1000);
    let wire = serde_json::to_value(cache.snapshot(1000)).unwrap();
    assert_eq!(wire["schemaVersion"], 28);
    assert_eq!(
        wire["diskHealth"]["value"]["totalWrittenBytes"],
        "123456789012345678901234"
    );
    assert_eq!(
        cache
            .snapshot(1000 + DISK_HEALTH_FRESHNESS_MS)
            .disk_health
            .status,
        MetricStatus::Ready
    );
    assert_eq!(
        cache
            .snapshot(1001 + DISK_HEALTH_FRESHNESS_MS)
            .disk_health
            .status,
        MetricStatus::Stale
    );
    cache.disk_health(Ok(health("other")), 2000);
    assert!(cache.snapshot(2000).disk_health.value.is_none());
    cache.disk_health(Ok(health("one")), 3000);
    cache.disk(volume("other"), capacity, 3001);
    assert!(cache.snapshot(3001).disk_health.value.is_none());
    cache.disk_health(Ok(health("other")), 4000);
    cache.fail(super::metrics::MetricId::Disk, MetricStatus::Disconnected);
    assert_eq!(
        cache.snapshot(4001).disk_health.status,
        MetricStatus::Disconnected
    );
    assert!(cache.snapshot(4001).disk_health.value.is_none());
}
