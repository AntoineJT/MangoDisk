//! Current preference models with strict validation and explicit release upgrades.
use mangodisk_core::system_resources::metrics::MetricId;
use serde::{Deserialize, Serialize};

mod migrations;
pub use migrations::decode;

pub(super) const PREFERENCE_SCHEMA_VERSION: u32 = 28;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskbarPosition {
    #[default]
    Auto,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FanDisplayMode {
    #[default]
    Highest,
    Average,
    Selected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum GpuDisplaySelection {
    #[default]
    Automatic,
    Selected {
        #[serde(rename = "adapterIds")]
        adapter_ids: Vec<String>,
    },
}

impl GpuDisplaySelection {
    fn valid(&self) -> bool {
        match self {
            Self::Selected { adapter_ids } => {
                !adapter_ids.is_empty()
                    && adapter_ids.len() <= 32
                    && adapter_ids
                        .iter()
                        .all(|id| !id.is_empty() && id.len() <= 1024 && !id.contains('\0'))
                    && adapter_ids
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == adapter_ids.len()
            }
            _ => true,
        }
    }
}

/// Display selection is independent of the five resource sampling workers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayMetricId {
    Cpu,
    CpuTemperature,
    Gpu,
    GpuTemperature,
    Memory,
    Disk,
    DiskTemperature,
    Network,
    BatteryTemperature,
    ChargingPower,
    BatteryPower,
    SystemPower,
    FanSpeed,
}

impl DisplayMetricId {
    pub fn metric(self) -> Option<MetricId> {
        match self {
            Self::Cpu => Some(MetricId::Cpu),
            Self::DiskTemperature
            | Self::CpuTemperature
            | Self::GpuTemperature
            | Self::BatteryTemperature
            | Self::ChargingPower
            | Self::BatteryPower
            | Self::SystemPower
            | Self::FanSpeed => None,
            Self::Gpu => Some(MetricId::Gpu),
            Self::Memory => Some(MetricId::Memory),
            Self::Disk => Some(MetricId::Disk),
            Self::Network => Some(MetricId::Network),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplayMetric {
    pub id: DisplayMetricId,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentPreferences {
    pub schema_version: u32,
    pub revision: u64,
    pub windows_cpu_temperature_enabled: bool,
    pub enabled: bool,
    pub show_icon: bool,
    pub taskbar_position: TaskbarPosition,
    pub taskbar_background: bool,
    pub taskbar_compact: bool,
    pub menu_bar_compact: bool,
    pub usage_colors: bool,
    pub color_rules: Vec<super::color_rules::MetricColorRule>,
    pub metrics: Vec<DisplayMetric>,
    pub network_interface: Option<String>,
    pub disk_volume: Option<String>,
    pub disk_temperature_device: Option<String>,
    pub gpu_adapter: Option<String>,
    pub gpu_display: GpuDisplaySelection,
    pub gpu_temperature_display: GpuDisplaySelection,
    pub fan_display: FanDisplayMode,
    pub fan_ids: Vec<u8>,
}

const DISPLAY_ORDER: [DisplayMetricId; 13] = [
    DisplayMetricId::Cpu,
    DisplayMetricId::CpuTemperature,
    DisplayMetricId::Gpu,
    DisplayMetricId::GpuTemperature,
    DisplayMetricId::Memory,
    DisplayMetricId::Disk,
    DisplayMetricId::DiskTemperature,
    DisplayMetricId::BatteryTemperature,
    DisplayMetricId::ChargingPower,
    DisplayMetricId::BatteryPower,
    DisplayMetricId::SystemPower,
    DisplayMetricId::FanSpeed,
    DisplayMetricId::Network,
];

impl Default for ResidentPreferences {
    fn default() -> Self {
        Self {
            schema_version: PREFERENCE_SCHEMA_VERSION,
            revision: 0,
            windows_cpu_temperature_enabled: false,
            fan_display: FanDisplayMode::Highest,
            fan_ids: vec![0],
            enabled: true,
            show_icon: true,
            taskbar_position: TaskbarPosition::default(),
            taskbar_background: true,
            taskbar_compact: false,
            menu_bar_compact: false,
            usage_colors: true,
            color_rules: super::color_rules::defaults(),
            metrics: DISPLAY_ORDER
                .into_iter()
                .map(|id| DisplayMetric {
                    id,
                    enabled: matches!(id, DisplayMetricId::Cpu | DisplayMetricId::Memory),
                })
                .collect(),
            network_interface: None,
            disk_volume: None,
            disk_temperature_device: None,
            gpu_adapter: None,
            gpu_display: GpuDisplaySelection::default(),
            gpu_temperature_display: GpuDisplaySelection::default(),
        }
    }
}

impl ResidentPreferences {
    pub fn shows(&self, id: MetricId) -> bool {
        self.metrics
            .iter()
            .any(|metric| metric.id.metric() == Some(id) && metric.enabled)
    }

    pub fn effective_icon(&self) -> bool {
        self.show_icon
            || !self.metrics.iter().any(|metric| {
                metric.enabled
                    && match metric.id {
                        DisplayMetricId::CpuTemperature => self.temperature_display_supported(),
                        DisplayMetricId::DiskTemperature | DisplayMetricId::GpuTemperature => {
                            cfg!(any(target_os = "macos", windows))
                        }
                        DisplayMetricId::BatteryTemperature
                        | DisplayMetricId::ChargingPower
                        | DisplayMetricId::BatteryPower => cfg!(any(target_os = "macos", windows)),
                        DisplayMetricId::FanSpeed | DisplayMetricId::SystemPower => {
                            cfg!(target_os = "macos")
                        }
                        _ => true,
                    }
            })
    }

    fn temperature_display_supported(&self) -> bool {
        cfg!(target_os = "macos") || (cfg!(windows) && self.windows_cpu_temperature_enabled)
    }
    pub fn shows_cpu_temperature(&self) -> bool {
        self.temperature_display_supported()
            && self
                .metrics
                .iter()
                .any(|metric| metric.id == DisplayMetricId::CpuTemperature && metric.enabled)
    }

    pub fn shows_disk_temperature(&self) -> bool {
        cfg!(any(target_os = "macos", windows))
            && self
                .metrics
                .iter()
                .any(|metric| metric.id == DisplayMetricId::DiskTemperature && metric.enabled)
    }

    pub fn shows_gpu_temperature(&self) -> bool {
        cfg!(any(target_os = "macos", windows))
            && self
                .metrics
                .iter()
                .any(|metric| metric.id == DisplayMetricId::GpuTemperature && metric.enabled)
    }

    pub fn shows_battery_item(&self, id: DisplayMetricId) -> bool {
        cfg!(any(target_os = "macos", windows))
            && self
                .metrics
                .iter()
                .any(|metric| metric.id == id && metric.enabled)
    }
    pub fn shows_fan_speed(&self) -> bool {
        cfg!(target_os = "macos")
            && self
                .metrics
                .iter()
                .any(|metric| metric.id == DisplayMetricId::FanSpeed && metric.enabled)
    }

    pub fn shows_system_power(&self) -> bool {
        cfg!(target_os = "macos")
            && self
                .metrics
                .iter()
                .any(|metric| metric.id == DisplayMetricId::SystemPower && metric.enabled)
    }

    pub fn shows_battery(&self) -> bool {
        self.shows_battery_item(DisplayMetricId::BatteryTemperature)
            || self.shows_battery_item(DisplayMetricId::ChargingPower)
            || self.shows_battery_item(DisplayMetricId::BatteryPower)
    }

    pub fn normalize(mut self) -> Result<Self, &'static str> {
        if self.schema_version != PREFERENCE_SCHEMA_VERSION {
            return Err("preferences_version");
        }
        if !super::color_rules::valid(&self.color_rules)
            || self.metrics.len() != DISPLAY_ORDER.len()
            || self.fan_ids.is_empty()
            || self.fan_ids.len() > 2
            || self
                .fan_ids
                .iter()
                .any(|id| *id >= mangodisk_platform::system_resources::fans::MAX_FANS)
            || (self.fan_ids.len() == 2 && self.fan_ids[0] == self.fan_ids[1])
            || [
                &self.network_interface,
                &self.disk_volume,
                &self.disk_temperature_device,
                &self.gpu_adapter,
            ]
            .iter()
            .any(|id| {
                id.as_ref()
                    .is_some_and(|id| id.is_empty() || id.len() > 1024 || id.contains('\0'))
            })
        {
            return Err("preferences_invalid");
        }
        if !self.gpu_display.valid() || !self.gpu_temperature_display.valid() {
            return Err("preferences_invalid");
        }
        self.fan_ids.sort_unstable();
        for id in DISPLAY_ORDER {
            if self.metrics.iter().filter(|metric| metric.id == id).count() != 1 {
                return Err("preferences_invalid");
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
#[path = "preference_schema_tests.rs"]
mod tests;
