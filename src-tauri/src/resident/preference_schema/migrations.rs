//! Backward-compatible preference decoding with no persistence or native UI effects.
use super::{DisplayMetricId, ResidentPreferences, TaskbarPosition, PREFERENCE_SCHEMA_VERSION};
use crate::resident::color_rules;
use serde::Deserialize;

pub(super) const PREVIOUS_DEFAULT_ORDER: [DisplayMetricId; 11] = [
    DisplayMetricId::Cpu,
    DisplayMetricId::CpuTemperature,
    DisplayMetricId::Gpu,
    DisplayMetricId::GpuTemperature,
    DisplayMetricId::Memory,
    DisplayMetricId::Disk,
    DisplayMetricId::Network,
    DisplayMetricId::BatteryTemperature,
    DisplayMetricId::ChargingPower,
    DisplayMetricId::BatteryPower,
    DisplayMetricId::SystemPower,
];

pub fn decode(mut value: serde_json::Value) -> Result<ResidentPreferences, &'static str> {
    let version = value.get("schemaVersion").and_then(|v| v.as_u64());
    if version.is_some_and(|version| version < 23)
        && (value.get("fanId").is_some_and(|value| !value.is_null())
            || value
                .get("metrics")
                .and_then(|value| value.as_array())
                .is_some_and(|metrics| metrics.iter().any(|metric| metric["id"] == "fanSpeed")))
    {
        return Err("preferences_invalid");
    }
    if matches!(version, Some(2..=24)) {
        if let Some(rules) = value
            .get_mut("colorRules")
            .and_then(|value| value.as_array_mut())
        {
            if rules.iter().any(|rule| rule["id"] == "fanSpeed") {
                return Err("preferences_invalid");
            }
            if version.is_some_and(|version| version >= 16) {
                rules.push(
                    serde_json::to_value(color_rules::MetricColorRule::defaults(
                        DisplayMetricId::FanSpeed,
                    ))
                    .map_err(|_| "preferences_invalid")?,
                );
            }
        }
    }
    if matches!(version, Some(2..=23)) {
        if value.get("fanDisplay").is_some() || value.get("fanIds").is_some() {
            return Err("preferences_invalid");
        }
        let old_id = value
            .as_object_mut()
            .ok_or("preferences_invalid")?
            .remove("fanId");
        let selected = match old_id {
            None | Some(serde_json::Value::Null) => None,
            Some(id) => Some(
                id.as_u64()
                    .filter(|id| *id < 10)
                    .ok_or("preferences_invalid")?,
            ),
        };
        value["fanDisplay"] = if selected.is_some() {
            "selected"
        } else {
            "highest"
        }
        .into();
        value["fanIds"] = serde_json::json!([selected.unwrap_or(0)]);
    }
    if matches!(version, Some(2..=21)) {
        if value
            .get("metrics")
            .and_then(|value| value.as_array())
            .is_some_and(|metrics| {
                metrics.iter().any(|metric| {
                    metric.get("id").and_then(|id| id.as_str()) == Some("diskTemperature")
                })
            })
        {
            return Err("preferences_invalid");
        }
        if let Some(rules) = value
            .get_mut("colorRules")
            .and_then(|value| value.as_array_mut())
        {
            if rules
                .iter()
                .any(|rule| rule.get("id").and_then(|id| id.as_str()) == Some("diskTemperature"))
            {
                return Err("preferences_invalid");
            }
            if version.is_some_and(|version| version >= 16) {
                rules.push(
                    serde_json::to_value(color_rules::MetricColorRule::defaults(
                        DisplayMetricId::DiskTemperature,
                    ))
                    .map_err(|_| "preferences_invalid")?,
                );
            }
        }
    }
    if matches!(version, Some(2..=12))
        && value
            .get("metrics")
            .and_then(|v| v.as_array())
            .is_some_and(|metrics| {
                metrics.iter().any(|metric| {
                    metric.get("id").and_then(|id| id.as_str()) == Some("gpuTemperature")
                })
            })
    {
        return Err("preferences_invalid");
    }
    if matches!(version, Some(2..=11)) {
        if value.get("windowsCpuTemperatureEnabled").is_some() {
            return Err("preferences_invalid");
        }
        value["windowsCpuTemperatureEnabled"] = false.into();
    }
    if matches!(version, Some(2..=10))
        && value
            .get("metrics")
            .and_then(|v| v.as_array())
            .is_some_and(|metrics| {
                metrics.iter().any(|metric| {
                    metric.get("id").and_then(|id| id.as_str()) == Some("cpuTemperature")
                })
            })
    {
        return Err("preferences_invalid");
    }
    if matches!(version, Some(2..=9)) {
        if value.get("gpuAdapter").is_some() {
            return Err("preferences_invalid");
        }
        value["gpuAdapter"] = serde_json::Value::Null;
    }
    if matches!(version, Some(2..=7)) {
        for field in [
            "menuBarCompact",
            "usageColors",
            "usageWarningPercent",
            "usageCriticalPercent",
        ] {
            if value.get(field).is_some() {
                return Err("preferences_invalid");
            }
        }
        // Upgrade only the former default sequence; keep deliberate custom order.
        if let Some(metrics) = value.get_mut("metrics").and_then(|v| v.as_array_mut()) {
            let ids: Vec<_> = metrics
                .iter()
                .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                .collect();
            if ids == ["cpu", "memory", "network", "disk"] {
                metrics.swap(2, 3);
            }
        }
        value["menuBarCompact"] = false.into();
        value["usageColors"] = true.into();
        value["usageWarningPercent"] = 70.into();
        value["usageCriticalPercent"] = 90.into();
    }
    // Versions 2–6 had no density preference. Preserve their appearance and
    // reject fields that did not belong to the declared legacy protocol.
    if matches!(
        value.get("schemaVersion").and_then(|v| v.as_u64()),
        Some(2..=6)
    ) {
        if value.get("taskbarCompact").is_some() {
            return Err("preferences_invalid");
        }
        value["taskbarCompact"] = false.into();
    }
    let mut preferences = match value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
    {
        Some(1) => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Previous {
                schema_version: u32,
                enabled: bool,
                show_memory: bool,
            }
            let previous: Previous =
                serde_json::from_value(value).map_err(|_| "preferences_invalid")?;
            if previous.schema_version != 1 {
                return Err("preferences_version");
            }
            let mut migrated = ResidentPreferences {
                enabled: previous.enabled,
                color_rules: color_rules::legacy(70, 90),
                // Preserve legacy metric and position choices. Windows now uses
                // taskbar presentation for every supported preference version.
                taskbar_position: TaskbarPosition::Right,
                ..Default::default()
            };
            for metric in &mut migrated.metrics {
                metric.enabled = metric.id == DisplayMetricId::Memory && previous.show_memory;
            }
            Ok(migrated)
        }
        Some(2) => {
            // Version 2 predates taskbar settings. Retain metric choices and
            // migrate the display destination to the sole supported taskbar mode.
            if value.get("windowsDisplayMode").is_some()
                || value.get("taskbarPosition").is_some()
                || value.get("taskbarBackground").is_some()
            {
                return Err("preferences_invalid");
            }
            value["schemaVersion"] = 14.into();
            value["taskbarBackground"] = true.into();
            value["windowsDisplayMode"] = "tray".into();
            value["taskbarPosition"] = "right".into();
            migrate_display_destination(value)
        }
        Some(3) => {
            if value.get("taskbarPosition").is_some() || value.get("taskbarBackground").is_some() {
                return Err("preferences_invalid");
            }
            value["schemaVersion"] = 14.into();
            value["taskbarBackground"] = true.into();
            value["taskbarPosition"] = "right".into();
            migrate_display_destination(value)
        }
        Some(4) => {
            if !matches!(
                value.get("taskbarPosition").and_then(|v| v.as_str()),
                Some("left" | "right")
            ) {
                return Err("preferences_invalid");
            }
            if value.get("taskbarBackground").is_some() {
                return Err("preferences_invalid");
            }
            // Existing installations keep the opaque presentation until the
            // user explicitly selects transparency.
            value["schemaVersion"] = 14.into();
            value["taskbarBackground"] = true.into();
            migrate_display_destination(value)
        }
        Some(5) => {
            // Version 5 predates automatic placement. Do not infer whether a saved
            // left/right value was a default or an explicit user decision.
            if !matches!(
                value.get("taskbarPosition").and_then(|v| v.as_str()),
                Some("left" | "right")
            ) {
                return Err("preferences_invalid");
            }
            value["schemaVersion"] = 14.into();
            migrate_display_destination(value)
        }
        Some(6) => {
            value["schemaVersion"] = 14.into();
            migrate_display_destination(value)
        }
        Some(7..=13) => {
            value["schemaVersion"] = 14.into();
            migrate_display_destination(value)
        }
        Some(14) => {
            if value.get("windowsDisplayMode").is_some() {
                return Err("preferences_invalid");
            }
            migrate_display_destination(value)
        }
        Some(15) => {
            let rules = color_rules::migrate_bands(
                value
                    .get("colorRules")
                    .cloned()
                    .ok_or("preferences_invalid")?,
            )?;
            value["colorRules"] = serde_json::to_value(rules).map_err(|_| "preferences_invalid")?;
            value["schemaVersion"] = 16.into();
            migrate_battery(value)
        }
        Some(16 | 17) => migrate_battery(value),
        Some(18) => migrate_system_power(value),
        Some(19) => {
            let mut preferences = serde_json::from_value::<ResidentPreferences>(value)
                .map_err(|_| "preferences_invalid")?;
            upgrade_battery_temperature_default(&mut preferences);
            preferences.schema_version = PREFERENCE_SCHEMA_VERSION;
            preferences.normalize()
        }
        Some(20..=24) => {
            let mut preferences = serde_json::from_value::<ResidentPreferences>(value)
                .map_err(|_| "preferences_invalid")?;
            preferences.schema_version = PREFERENCE_SCHEMA_VERSION;
            preferences.normalize()
        }
        Some(25) => serde_json::from_value::<ResidentPreferences>(value)
            .map_err(|_| "preferences_invalid")?
            .normalize(),
        _ => Err("preferences_version"),
    }?;
    // Upgrade the exact former default once. Version-21 custom orders, including
    // ones that resemble the old default, must remain stable across reloads.
    if version.is_some_and(|version| version < 21)
        && preferences
            .metrics
            .iter()
            .map(|metric| metric.id)
            .filter(|id| {
                !matches!(
                    id,
                    DisplayMetricId::DiskTemperature | DisplayMetricId::FanSpeed
                )
            })
            .eq(PREVIOUS_DEFAULT_ORDER)
    {
        let index = preferences
            .metrics
            .iter()
            .position(|metric| metric.id == DisplayMetricId::Network)
            .unwrap();
        let network = preferences.metrics.remove(index);
        preferences.metrics.push(network);
    }
    Ok(preferences)
}

fn upgrade_battery_temperature_default(preferences: &mut ResidentPreferences) {
    if let Some(rule) = preferences.color_rules.iter_mut().find(|rule| {
        rule.id == DisplayMetricId::BatteryTemperature && !rule.enabled && rule.rules.is_empty()
    }) {
        // Match only the old inactive, empty default. Saved custom rules and
        // version-20 users who deliberately clear rules must remain unchanged.
        *rule = color_rules::MetricColorRule::defaults(DisplayMetricId::BatteryTemperature);
    }
}

fn migrate_system_power(value: serde_json::Value) -> Result<ResidentPreferences, &'static str> {
    let mut preferences =
        serde_json::from_value::<ResidentPreferences>(value).map_err(|_| "preferences_invalid")?;
    if preferences
        .metrics
        .iter()
        .any(|metric| metric.id == DisplayMetricId::SystemPower)
    {
        return Err("preferences_invalid");
    }
    // Only append the new rule. Existing malformed version-18 rules must still
    // fail validation instead of being silently repaired and persisted.
    preferences
        .color_rules
        .push(color_rules::MetricColorRule::defaults(
            DisplayMetricId::SystemPower,
        ));
    preferences.schema_version = PREFERENCE_SCHEMA_VERSION;
    upgrade_battery_temperature_default(&mut preferences);
    preferences.normalize()
}

fn migrate_battery(mut value: serde_json::Value) -> Result<ResidentPreferences, &'static str> {
    let rules = value
        .get_mut("colorRules")
        .and_then(|value| value.as_array_mut())
        .ok_or("preferences_invalid")?;
    for id in [
        DisplayMetricId::BatteryTemperature,
        DisplayMetricId::ChargingPower,
        DisplayMetricId::BatteryPower,
        DisplayMetricId::SystemPower,
    ] {
        let name = serde_json::to_value(id).map_err(|_| "preferences_invalid")?;
        if !rules.iter().any(|rule| rule.get("id") == Some(&name)) {
            rules.push(
                serde_json::to_value(color_rules::MetricColorRule::defaults(id))
                    .map_err(|_| "preferences_invalid")?,
            );
        }
    }
    value["schemaVersion"] = PREFERENCE_SCHEMA_VERSION.into();
    let mut preferences =
        serde_json::from_value::<ResidentPreferences>(value).map_err(|_| "preferences_invalid")?;
    upgrade_battery_temperature_default(&mut preferences);
    preferences.normalize()
}

fn migrate_display_destination(
    mut value: serde_json::Value,
) -> Result<ResidentPreferences, &'static str> {
    if let Some(mode) = value
        .as_object_mut()
        .and_then(|object| object.remove("windowsDisplayMode"))
    {
        if !matches!(mode.as_str(), Some("taskbar" | "tray")) {
            return Err("preferences_invalid");
        }
    }
    if value.get("colorRules").is_some() {
        return Err("preferences_invalid");
    }
    let warning = value
        .get("usageWarningPercent")
        .and_then(|v| v.as_u64())
        .ok_or("preferences_invalid")?;
    let critical = value
        .get("usageCriticalPercent")
        .and_then(|v| v.as_u64())
        .ok_or("preferences_invalid")?;
    if warning < 1 || warning >= critical || critical > 100 {
        return Err("preferences_invalid");
    }
    value
        .as_object_mut()
        .ok_or("preferences_invalid")?
        .remove("usageWarningPercent");
    value
        .as_object_mut()
        .ok_or("preferences_invalid")?
        .remove("usageCriticalPercent");
    value["colorRules"] =
        serde_json::to_value(color_rules::legacy(warning as u16, critical as u16))
            .map_err(|_| "preferences_invalid")?;
    value["schemaVersion"] = 16.into();
    migrate_battery(value)
}
