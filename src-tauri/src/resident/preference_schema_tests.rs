use super::*;
#[test]
fn version_twenty_four_adds_only_fan_color_defaults_and_rejects_malformed_rules() {
    use super::super::color_rules::{ColorRule, DisplayColor};
    let mut expected = ResidentPreferences {
        revision: 42,
        fan_display: FanDisplayMode::Selected,
        fan_ids: vec![0, 1],
        ..Default::default()
    };
    expected.color_rules[0].rules = vec![ColorRule::AtLeast {
        value: 30,
        color: DisplayColor::Blue,
    }];
    let mut previous = serde_json::to_value(&expected).unwrap();
    previous["schemaVersion"] = 24.into();
    previous["colorRules"]
        .as_array_mut()
        .unwrap()
        .retain(|rule| rule["id"] != "fanSpeed");
    assert_eq!(super::decode(previous.clone()).unwrap(), expected);
    let mut invalid = previous.clone();
    invalid["colorRules"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(expected.color_rules.last().unwrap()).unwrap());
    assert!(super::decode(invalid).is_err());
    let mut invalid = previous.clone();
    invalid["colorRules"][0]["rules"][0]["value"] = 101.into();
    assert!(super::decode(invalid).is_err());
    let mut invalid = previous.clone();
    invalid["colorRules"].as_array_mut().unwrap().remove(0);
    assert!(super::decode(invalid).is_err());
    previous["schemaVersion"] = 25.into();
    assert!(super::decode(previous).is_err());
    assert_eq!(
        super::decode(serde_json::to_value(&expected).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn fan_speed_migration_preserves_saved_order_colors_and_manual_source_choices() {
    let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
    old["schemaVersion"] = 22.into();
    old["colorRules"]
        .as_array_mut()
        .unwrap()
        .retain(|rule| rule["id"] != "fanSpeed");
    old.as_object_mut().unwrap().remove("fanDisplay");
    old.as_object_mut().unwrap().remove("fanIds");
    let rows = old["metrics"].as_array_mut().unwrap();
    rows.retain(|row| row["id"] != "fanSpeed");
    rows.reverse();
    old["diskVolume"] = "saved-volume".into();
    old["networkInterface"] = "saved-interface".into();
    let before = old.clone();
    let migrated = super::decode(old).unwrap();
    assert_eq!(migrated.schema_version, 25);
    assert_eq!(migrated.fan_display, FanDisplayMode::Highest);
    assert_eq!(migrated.fan_ids, vec![0]);
    let value = serde_json::to_value(&migrated).unwrap();
    let rows: Vec<_> = value["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["id"] != "fanSpeed")
        .cloned()
        .collect();
    assert_eq!(serde_json::Value::Array(rows), before["metrics"]);
    assert_eq!(
        serde_json::Value::Array(
            value["colorRules"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|rule| rule["id"] != "fanSpeed")
                .cloned()
                .collect()
        ),
        before["colorRules"]
    );
    assert_eq!(value["diskVolume"], before["diskVolume"]);
    assert_eq!(value["networkInterface"], before["networkInterface"]);
    assert!(
        !migrated
            .metrics
            .iter()
            .find(|row| row.id == DisplayMetricId::FanSpeed)
            .unwrap()
            .enabled
    );
    assert_eq!(super::decode(value).unwrap(), migrated);
}

#[test]
fn fan_pair_migration_bounds_and_platform_visibility_are_explicit() {
    for (id, mode, ids) in [
        (serde_json::json!(null), FanDisplayMode::Highest, vec![0]),
        (serde_json::json!(9), FanDisplayMode::Selected, vec![9]),
    ] {
        let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
        old["schemaVersion"] = 23.into();
        old["colorRules"]
            .as_array_mut()
            .unwrap()
            .retain(|rule| rule["id"] != "fanSpeed");
        old.as_object_mut().unwrap().remove("fanDisplay");
        old.as_object_mut().unwrap().remove("fanIds");
        old["fanId"] = id;
        let migrated = super::decode(old).unwrap();
        assert_eq!(migrated.fan_display, mode);
        assert_eq!(migrated.fan_ids, ids);
    }
    for ids in [vec![], vec![0, 1, 2], vec![0, 0], vec![10]] {
        let preferences = ResidentPreferences {
            fan_ids: ids,
            ..Default::default()
        };
        assert_eq!(preferences.normalize().unwrap_err(), "preferences_invalid");
    }
    let preferences = ResidentPreferences {
        fan_ids: vec![9, 0],
        ..Default::default()
    }
    .normalize()
    .unwrap();
    assert_eq!(preferences.fan_ids, vec![0, 9]);
    assert_eq!(
        decode(serde_json::to_value(&preferences).unwrap()).unwrap(),
        preferences
    );
    for old_id in [
        serde_json::json!(-1),
        serde_json::json!(10),
        serde_json::json!(0.5),
        serde_json::json!("0"),
    ] {
        let mut old = serde_json::to_value(&preferences).unwrap();
        old["schemaVersion"] = 23.into();
        old["colorRules"]
            .as_array_mut()
            .unwrap()
            .retain(|rule| rule["id"] != "fanSpeed");
        old.as_object_mut().unwrap().remove("fanDisplay");
        old.as_object_mut().unwrap().remove("fanIds");
        old["fanId"] = old_id;
        assert_eq!(decode(old).unwrap_err(), "preferences_invalid");
    }
    let mut preferences = ResidentPreferences {
        show_icon: false,
        ..Default::default()
    };
    for row in &mut preferences.metrics {
        row.enabled = row.id == DisplayMetricId::FanSpeed;
    }
    assert_eq!(preferences.shows_fan_speed(), cfg!(target_os = "macos"));
    assert_eq!(preferences.effective_icon(), !cfg!(target_os = "macos"));
    let mut old = serde_json::to_value(&preferences).unwrap();
    old["schemaVersion"] = 22.into();
    assert_eq!(decode(old).unwrap_err(), "preferences_invalid");
}

#[test]
fn disk_temperature_migrates_after_disk_disabled_without_changing_saved_settings() {
    let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
    old["schemaVersion"] = 21.into();
    old["colorRules"]
        .as_array_mut()
        .unwrap()
        .retain(|rule| rule["id"] != "fanSpeed");
    old.as_object_mut().unwrap().remove("fanDisplay");
    old.as_object_mut().unwrap().remove("fanIds");
    old["metrics"]
        .as_array_mut()
        .unwrap()
        .retain(|row| row["id"] != "fanSpeed");
    old["networkInterface"] = "saved-vpn".into();
    old["diskVolume"] = "saved-volume".into();
    for field in ["metrics", "colorRules"] {
        old[field]
            .as_array_mut()
            .unwrap()
            .retain(|row| row["id"] != "diskTemperature");
    }
    old["metrics"].as_array_mut().unwrap().reverse();
    let before = old.clone();
    let migrated = super::decode(old).unwrap();
    let disk = migrated
        .metrics
        .iter()
        .position(|row| row.id == DisplayMetricId::Disk)
        .unwrap();
    assert_eq!(
        migrated.metrics[disk + 1],
        DisplayMetric {
            id: DisplayMetricId::DiskTemperature,
            enabled: false
        }
    );
    let current = serde_json::to_value(&migrated).unwrap();
    assert_eq!(previous_rows(&current["metrics"]), before["metrics"]);
    assert_eq!(previous_rows(&current["colorRules"]), before["colorRules"]);
    assert_eq!(migrated.network_interface.as_deref(), Some("saved-vpn"));
    assert_eq!(migrated.disk_volume.as_deref(), Some("saved-volume"));
    assert_eq!(
        migrated
            .color_rules
            .iter()
            .find(|rule| rule.id == DisplayMetricId::DiskTemperature),
        Some(&super::super::color_rules::MetricColorRule::defaults(
            DisplayMetricId::DiskTemperature
        ))
    );
    assert_eq!(super::decode(current).unwrap(), migrated);
    let mut invalid = before;
    invalid["metrics"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"diskTemperature", "enabled":true}));
    assert_eq!(super::decode(invalid).unwrap_err(), "preferences_invalid");
}

#[test]
fn saved_disk_temperature_colors_remain_explicit_including_disabled_empty_rules() {
    use super::super::color_rules::{ColorRule, DisplayColor};
    for enabled in [false, true] {
        for custom in [false, true] {
            let mut preferences = ResidentPreferences::default();
            let rule = preferences
                .color_rules
                .iter_mut()
                .find(|rule| rule.id == DisplayMetricId::DiskTemperature)
                .unwrap();
            rule.enabled = enabled;
            rule.rules = if custom {
                vec![ColorRule::AtLeast {
                    value: 80,
                    color: DisplayColor::Blue,
                }]
            } else {
                vec![]
            };
            assert_eq!(
                super::decode(serde_json::to_value(&preferences).unwrap()).unwrap(),
                preferences
            );
        }
    }
}

fn previous_rows(value: &serde_json::Value) -> serde_json::Value {
    serde_json::Value::Array(
        value
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["id"] != "diskTemperature" && row["id"] != "fanSpeed")
            .cloned()
            .collect(),
    )
}
fn decode_fixture(mut value: serde_json::Value) -> Result<ResidentPreferences, &'static str> {
    if value["schemaVersion"]
        .as_u64()
        .is_some_and(|version| version < 25)
    {
        if let Some(rules) = value
            .get_mut("colorRules")
            .and_then(|value| value.as_array_mut())
        {
            rules.retain(|rule| rule["id"] != "fanSpeed");
        }
    }
    if value["schemaVersion"]
        .as_u64()
        .is_some_and(|version| version < 24)
    {
        value.as_object_mut().unwrap().remove("fanDisplay");
        value.as_object_mut().unwrap().remove("fanIds");
    }
    if value["schemaVersion"]
        .as_u64()
        .is_some_and(|version| version < 23)
    {
        value.as_object_mut().unwrap().remove("fanId");
        if let Some(rows) = value
            .get_mut("metrics")
            .and_then(|rows| rows.as_array_mut())
        {
            rows.retain(|row| row["id"] != "fanSpeed");
        }
    }
    if value
        .get("schemaVersion")
        .and_then(|value| value.as_u64())
        .is_some_and(|version| version < 22)
    {
        for field in ["metrics", "colorRules"] {
            if let Some(rows) = value.get_mut(field).and_then(|value| value.as_array_mut()) {
                rows.retain(|row| {
                    row.get("id").and_then(|id| id.as_str()) != Some("diskTemperature")
                });
            }
        }
    }
    super::decode(value)
}
#[test]
fn version_twenty_upgrades_only_the_complete_previous_default_order() {
    for custom in [false, true] {
        let mut previous = ResidentPreferences {
            schema_version: 20,
            revision: 123,
            metrics: PREVIOUS_DEFAULT_ORDER
                .into_iter()
                .enumerate()
                .map(|(index, id)| DisplayMetric {
                    id,
                    enabled: index % 2 == 0,
                })
                .collect(),
            ..Default::default()
        };
        if custom {
            previous.metrics.swap(0, 1);
        }
        let mut expected = previous.clone();
        expected.schema_version = 25;
        if !custom {
            let network = expected.metrics.remove(6);
            expected.metrics.push(network);
        }
        let migrated = decode_fixture(serde_json::to_value(previous).unwrap()).unwrap();
        expected
            .color_rules
            .retain(|rule| rule.id != DisplayMetricId::DiskTemperature);
        expected
            .color_rules
            .push(super::super::color_rules::MetricColorRule::defaults(
                DisplayMetricId::DiskTemperature,
            ));
        assert_eq!(migrated, expected.normalize().unwrap());
        assert_eq!(
            decode_fixture(serde_json::to_value(&migrated).unwrap()).unwrap(),
            migrated
        );
    }
}
#[test]
fn current_custom_order_can_match_the_former_default_without_being_reordered() {
    let mut preferences = ResidentPreferences::default();
    preferences.metrics.sort_by_key(|metric| {
        PREVIOUS_DEFAULT_ORDER
            .iter()
            .position(|id| *id == metric.id)
            .unwrap_or(PREVIOUS_DEFAULT_ORDER.len())
    });
    assert_eq!(
        decode_fixture(serde_json::to_value(&preferences).unwrap()).unwrap(),
        preferences
    );
}
#[test]
fn missing_display_items_keep_a_trailing_network_last() {
    let expected = ResidentPreferences::default();
    assert_eq!(
        expected.metrics.last().unwrap().id,
        DisplayMetricId::Network
    );
    let mut partial = expected.clone();
    partial.metrics.retain(|metric| {
        !matches!(
            metric.id,
            DisplayMetricId::BatteryTemperature
                | DisplayMetricId::ChargingPower
                | DisplayMetricId::BatteryPower
                | DisplayMetricId::SystemPower
                | DisplayMetricId::FanSpeed
        )
    });
    assert_eq!(partial.normalize().unwrap(), expected);
}
#[test]
fn version_nineteen_upgrades_only_the_old_empty_battery_temperature_default() {
    use super::super::color_rules::{ColorRule, DisplayColor, MetricColorRule};
    for custom in [false, true] {
        let mut preferences = ResidentPreferences {
            schema_version: 19,
            ..Default::default()
        };
        let rule = preferences
            .color_rules
            .iter_mut()
            .find(|rule| rule.id == DisplayMetricId::BatteryTemperature)
            .unwrap();
        rule.enabled = false;
        rule.rules = if custom {
            vec![ColorRule::AtLeast {
                value: 50,
                color: DisplayColor::Blue,
            }]
        } else {
            vec![]
        };
        let before = preferences.clone();
        let migrated = decode_fixture(serde_json::to_value(preferences).unwrap()).unwrap();
        let current = migrated
            .color_rules
            .iter()
            .find(|rule| rule.id == DisplayMetricId::BatteryTemperature)
            .unwrap();
        if custom {
            assert_eq!(
                current,
                before
                    .color_rules
                    .iter()
                    .find(|rule| rule.id == DisplayMetricId::BatteryTemperature)
                    .unwrap()
            );
        } else {
            assert_eq!(
                *current,
                MetricColorRule::defaults(DisplayMetricId::BatteryTemperature)
            );
            assert!(current.enabled);
        }
        assert_eq!(migrated.metrics, before.metrics);
        let mut cleared = migrated;
        let current = cleared
            .color_rules
            .iter_mut()
            .find(|rule| rule.id == DisplayMetricId::BatteryTemperature)
            .unwrap();
        current.enabled = false;
        current.rules.clear();
        assert_eq!(
            decode_fixture(serde_json::to_value(&cleared).unwrap()).unwrap(),
            cleared
        );
    }
}

#[test]
fn version_eighteen_does_not_repair_malformed_existing_color_rules() {
    let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
    old["schemaVersion"] = 18.into();
    old["metrics"]
        .as_array_mut()
        .unwrap()
        .retain(|row| row["id"] != "systemPower");
    old["colorRules"]
        .as_array_mut()
        .unwrap()
        .retain(|row| row["id"] != "systemPower" && row["id"] != "batteryPower");
    assert!(decode_fixture(old).is_err());
}
#[test]
fn version_eighteen_preserves_settings_and_appends_system_power_disabled() {
    let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
    old["schemaVersion"] = 18.into();
    old["revision"] = 123.into();
    for key in ["metrics", "colorRules"] {
        old[key]
            .as_array_mut()
            .unwrap()
            .retain(|row| row["id"] != "systemPower");
        old[key].as_array_mut().unwrap().reverse();
    }
    let before = old.clone();
    let migrated = serde_json::to_value(decode_fixture(old).unwrap()).unwrap();
    assert_eq!(migrated["revision"], before["revision"]);
    for key in ["metrics", "colorRules"] {
        let previous = previous_rows(&before[key]);
        let previous = previous.as_array().unwrap();
        let current = previous_rows(&migrated[key]);
        let current = current.as_array().unwrap();
        assert_eq!(&current[..previous.len()], previous);
        assert_eq!(current.last().unwrap()["id"], "systemPower");
        assert_eq!(current.last().unwrap()["enabled"], false);
    }
}

#[test]
fn actual_version_fifteen_six_color_metrics_migrate_with_empty_power_rules() {
    let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
    old["schemaVersion"] = 15.into();
    old["colorRules"] = serde_json::json!(["cpu", "cpuTemperature", "gpu", "gpuTemperature", "memory", "disk"])
            .as_array().unwrap().iter().map(|id| serde_json::json!({
                "id": id, "enabled": true,
                "bands": [{"from":0,"color":"default"},{"from":70,"color":"orange"},{"from":90,"color":"red"}]
            })).collect();
    let migrated = decode_fixture(old).unwrap();
    let power = migrated
        .color_rules
        .iter()
        .find(|rule| rule.id == DisplayMetricId::SystemPower)
        .unwrap();
    assert!(!power.enabled);
    assert!(power.rules.is_empty());
}

#[test]
fn version_seventeen_preserves_battery_selections_and_appends_battery_power_disabled() {
    let mut preferences = ResidentPreferences::default();
    for row in &mut preferences.metrics {
        if matches!(
            row.id,
            DisplayMetricId::BatteryTemperature | DisplayMetricId::ChargingPower
        ) {
            row.enabled = true;
        }
    }
    let mut old = serde_json::to_value(preferences).unwrap();
    old["schemaVersion"] = 17.into();
    for key in ["metrics", "colorRules"] {
        old[key].as_array_mut().unwrap().retain(|row| {
            row["id"] != "batteryPower"
                && row["id"] != "systemPower"
                && row["id"] != "diskTemperature"
        });
    }
    let old_metrics = old["metrics"].clone();
    let old_colors = old["colorRules"].clone();
    let migrated = decode_fixture(old).unwrap();
    assert!(
        migrated.shows_battery_item(DisplayMetricId::ChargingPower) || !cfg!(target_os = "macos")
    );
    let current = serde_json::to_value(migrated).unwrap();
    assert_eq!(
        current["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| {
                row["id"] != "batteryPower"
                    && row["id"] != "systemPower"
                    && row["id"] != "diskTemperature"
                    && row["id"] != "fanSpeed"
            })
            .collect::<Vec<_>>(),
        old_metrics
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["id"] != "diskTemperature" && row["id"] != "fanSpeed")
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &current["colorRules"].as_array().unwrap()[..8],
        previous_rows(&old_colors).as_array().unwrap()
    );
    for id in ["batteryPower", "systemPower"] {
        let row = current["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .unwrap();
        assert_eq!(row["enabled"], false);
    }
}
#[test]
fn version_sixteen_preserves_custom_colors_and_appends_disabled_battery_items() {
    let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
    old["schemaVersion"] = 16.into();
    for key in ["metrics", "colorRules"] {
        old[key].as_array_mut().unwrap().retain(|row| {
            !matches!(
                row["id"].as_str(),
                Some("batteryTemperature" | "chargingPower" | "batteryPower" | "systemPower")
            )
        });
    }
    old["revision"] = 123.into();
    old["metrics"].as_array_mut().unwrap().reverse();
    let old_metrics = old["metrics"].clone();
    let old_colors = old["colorRules"].clone();
    let migrated = decode_fixture(old).unwrap();
    assert_eq!(migrated.revision, 123);
    assert!(migrated
        .metrics
        .iter()
        .filter(|row| matches!(
            row.id,
            DisplayMetricId::BatteryTemperature
                | DisplayMetricId::ChargingPower
                | DisplayMetricId::BatteryPower
        ))
        .all(|row| !row.enabled));
    let current = serde_json::to_value(&migrated).unwrap();
    assert_eq!(
        &previous_rows(&current["metrics"]).as_array().unwrap()[..7],
        previous_rows(&old_metrics).as_array().unwrap()
    );
    assert_eq!(
        &current["colorRules"].as_array().unwrap()[..6],
        previous_rows(&old_colors).as_array().unwrap()
    );
    assert_eq!(decode_fixture(current).unwrap(), migrated);
}

#[test]
fn version_fifteen_migrates_colors_without_changing_selections_or_boundaries() {
    let mut old = serde_json::to_value(ResidentPreferences::default()).unwrap();
    old["schemaVersion"] = 15.into();
    old["colorRules"] = serde_json::Value::Array(super::super::color_rules::COLOR_METRICS.iter().map(|id| serde_json::json!({
            "id": id, "enabled": false, "bands": [{"from": 0, "color": "blue"}, {"from": 55, "color": "yellow"}, {"from": 95, "color": "red"}]
        })).collect());
    let migrated = decode_fixture(old.clone()).unwrap();
    assert_eq!(migrated.schema_version, 25);
    assert_eq!(
        previous_rows(&serde_json::to_value(&migrated.metrics).unwrap()),
        previous_rows(&old["metrics"])
    );
    for rule in migrated.color_rules.iter().filter(|rule| {
        !matches!(
            rule.id,
            DisplayMetricId::DiskTemperature | DisplayMetricId::FanSpeed
        )
    }) {
        assert!(!rule.enabled);
        assert_eq!(rule.rules[0].bounds(), (0, Some(55)));
        assert_eq!(rule.rules[1].bounds(), (55, Some(95)));
        assert_eq!(rule.rules[2].bounds(), (95, None));
        assert_eq!(
            rule.rules[0].color(),
            super::super::color_rules::DisplayColor::Blue
        );
    }
    old["colorRules"][0]["bands"][1]["from"] = 0.into();
    assert!(decode_fixture(old).is_err());
}

#[test]
fn current_rules_round_trip_empty_single_and_sparse_conditions() {
    let mut preferences = ResidentPreferences::default();
    preferences.color_rules[0].rules.clear();
    preferences.color_rules[1].rules.truncate(1);
    preferences.color_rules[2].rules.remove(1);
    assert_eq!(
        decode_fixture(serde_json::to_value(&preferences).unwrap()).unwrap(),
        preferences
    );
    let mut malformed = serde_json::to_value(&preferences).unwrap();
    malformed["colorRules"][1]["rules"][0]["from"] = 1.into();
    assert!(decode_fixture(malformed).is_err());
}

#[test]
fn current_rules_reject_unknown_colors_missing_metrics_and_legacy_fields() {
    let current = serde_json::to_value(ResidentPreferences::default()).unwrap();
    assert_eq!(decode_fixture(current.clone()).unwrap().schema_version, 25);
    let mut value = current.clone();
    value["colorRules"][0]["rules"][0]["color"] = "purple".into();
    assert!(decode_fixture(value).is_err());
    let mut value = current.clone();
    value["colorRules"].as_array_mut().unwrap().pop();
    assert!(decode_fixture(value).is_err());
    let mut value = current.clone();
    value["usageWarningPercent"] = 70.into();
    assert!(decode_fixture(value).is_err());
    let mut value = current;
    value["schemaVersion"] = 14.into();
    value["usageWarningPercent"] = 70.into();
    value["usageCriticalPercent"] = 90.into();
    assert!(decode_fixture(value).is_err());
}

#[test]
fn version_thirteen_removes_tray_destination_and_preserves_user_settings() {
    for mode in ["tray", "taskbar"] {
        let mut old = version_fourteen_preferences();
        old["schemaVersion"] = 13.into();
        old["windowsDisplayMode"] = mode.into();
        old["gpuAdapter"] = "selected-adapter".into();
        old["enabled"] = false.into();
        old["revision"] = 47.into();
        let migrated = decode_fixture(old).unwrap();
        assert_eq!(migrated.schema_version, 25);
        assert_eq!(migrated.revision, 47);
        assert!(!migrated.enabled);
        assert_eq!(migrated.gpu_adapter.as_deref(), Some("selected-adapter"));
        let serialized = serde_json::to_value(&migrated).unwrap();
        assert!(serialized.get("windowsDisplayMode").is_none());
        assert_eq!(decode_fixture(serialized).unwrap(), migrated);
    }
    let mut current = serde_json::to_value(ResidentPreferences::default()).unwrap();
    current["windowsDisplayMode"] = "tray".into();
    assert!(decode_fixture(current).is_err());
}

#[test]
fn version_twelve_preserves_gpu_selection_and_appends_unselected_temperature() {
    let mut value = version_fourteen_preferences();
    value["schemaVersion"] = 12.into();
    value["revision"] = 84.into();
    value["gpuAdapter"] = "fixed-gpu".into();
    let metrics = value["metrics"].as_array_mut().unwrap();
    metrics.retain(|metric| metric["id"] != "gpuTemperature");
    metrics.reverse();
    let mut expected = metrics.clone();
    expected.push(serde_json::json!({"id":"gpuTemperature", "enabled":false}));
    let migrated = decode_fixture(value.clone()).unwrap();
    assert_eq!(migrated.revision, 84);
    assert_eq!(migrated.gpu_adapter.as_deref(), Some("fixed-gpu"));
    assert!(!migrated.shows_gpu_temperature());
    assert_eq!(
        previous_rows(&serde_json::to_value(&migrated).unwrap()["metrics"]),
        previous_rows(&serde_json::json!(expected))
    );
    value["metrics"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"gpuTemperature", "enabled":true}));
    assert!(decode_fixture(value).is_err());
}

#[test]
fn version_eleven_never_opts_into_privileged_collection_during_migration() {
    let mut old = version_fourteen_preferences();
    old["metrics"]
        .as_array_mut()
        .unwrap()
        .retain(|m| m["id"] != "gpuTemperature");
    old["schemaVersion"] = 11.into();
    old["revision"] = 43.into();
    old["metrics"].as_array_mut().unwrap().reverse();
    old["metrics"][0]["enabled"] = true.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    let migrated = decode_fixture(old.clone()).unwrap();
    let mut expected = old["metrics"].clone();
    expected
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"gpuTemperature","enabled":false}));
    assert!(!migrated.windows_cpu_temperature_enabled);
    assert_eq!(migrated.revision, 43);
    assert_eq!(
        previous_rows(&serde_json::to_value(migrated).unwrap()["metrics"]),
        previous_rows(&expected)
    );
    old["windowsCpuTemperatureEnabled"] = true.into();
    assert!(decode_fixture(old).is_err());
}

#[test]
fn version_ten_preserves_choices_and_appends_disabled_temperature() {
    let mut old = legacy_current_preferences();
    old["schemaVersion"] = 10.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    old["revision"] = 42.into();
    old["metrics"].as_array_mut().unwrap().reverse();
    let migrated = decode_fixture(old.clone()).unwrap();
    assert_eq!(migrated.schema_version, 25);
    assert_eq!(migrated.revision, 42);
    let mut expected = old["metrics"].clone();
    expected
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"cpuTemperature","enabled":false}));
    expected
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"gpuTemperature","enabled":false}));
    assert_eq!(
        previous_rows(&serde_json::to_value(&migrated).unwrap()["metrics"]),
        previous_rows(&expected)
    );
    assert!(!migrated.shows_cpu_temperature());
    old["metrics"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"cpuTemperature","enabled":true}));
    assert!(decode_fixture(old).is_err());
    let mut selected = migrated;
    let last = selected.metrics.pop().unwrap();
    selected.metrics.insert(
        1,
        DisplayMetric {
            enabled: true,
            ..last
        },
    );
    assert_eq!(
        decode_fixture(serde_json::to_value(&selected).unwrap()).unwrap(),
        selected
    );
    let mut invalid = serde_json::to_value(selected).unwrap();
    invalid["metrics"][1]["enabled"] = "true".into();
    assert!(decode_fixture(invalid).is_err());
}

#[test]
fn temperature_only_suppresses_logo_fallback_only_on_macos() {
    let mut prefs = ResidentPreferences {
        show_icon: false,
        ..Default::default()
    };
    for metric in &mut prefs.metrics {
        metric.enabled = metric.id == DisplayMetricId::CpuTemperature;
    }
    assert_eq!(prefs.effective_icon(), !cfg!(target_os = "macos"));
    prefs
        .metrics
        .iter_mut()
        .for_each(|metric| metric.enabled = false);
    assert!(prefs.effective_icon());
}

fn legacy_current_preferences() -> serde_json::Value {
    let mut value = version_fourteen_preferences();
    value["metrics"]
        .as_array_mut()
        .unwrap()
        .retain(|metric| metric["id"] != "cpuTemperature" && metric["id"] != "gpuTemperature");
    value
}

fn legacy_preferences() -> serde_json::Value {
    let mut value = legacy_current_preferences();
    value.as_object_mut().unwrap().remove("gpuAdapter");
    value["schemaVersion"] = 7.into();
    value
        .as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    value["metrics"]
        .as_array_mut()
        .unwrap()
        .retain(|metric| metric["id"] != "gpu");
    for field in [
        "menuBarCompact",
        "usageColors",
        "usageWarningPercent",
        "usageCriticalPercent",
    ] {
        value.as_object_mut().unwrap().remove(field);
    }
    value
}

#[test]
fn version_nine_defaults_to_automatic_gpu_and_preserves_current_choices() {
    let mut old = legacy_current_preferences();
    old["schemaVersion"] = 9.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    old["networkInterface"] = "wifi".into();
    old.as_object_mut().unwrap().remove("gpuAdapter");
    let migrated = decode_fixture(old.clone()).unwrap();
    assert_eq!(migrated.schema_version, 25);
    assert_eq!(migrated.network_interface.as_deref(), Some("wifi"));
    assert!(migrated.gpu_adapter.is_none());
    old["gpuAdapter"] = "unexpected".into();
    assert!(decode_fixture(old).is_err());
    let fixed = ResidentPreferences {
        gpu_adapter: Some("pci:10de:2684:1:0:0:0".into()),
        ..migrated
    };
    assert_eq!(
        decode_fixture(serde_json::to_value(&fixed).unwrap()).unwrap(),
        fixed
    );
    assert!(ResidentPreferences {
        gpu_adapter: Some("".into()),
        ..fixed
    }
    .normalize()
    .is_err());
}
#[test]
fn version_eight_appends_disabled_gpu_without_changing_saved_choices() {
    let mut old = legacy_current_preferences();
    old["schemaVersion"] = 8.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    old.as_object_mut().unwrap().remove("gpuAdapter");
    old["revision"] = 42.into();
    old["metrics"] = serde_json::json!([
        {"id":"memory", "enabled":true}, {"id":"cpu", "enabled":false},
        {"id":"network", "enabled":true}, {"id":"disk", "enabled":false}
    ]);
    let migrated = decode_fixture(old).unwrap();
    assert_eq!(migrated.schema_version, 25);
    assert_eq!(migrated.revision, 42);
    assert_eq!(
        migrated
            .metrics
            .iter()
            .map(|metric| metric.id)
            .filter(|id| !matches!(
                id,
                DisplayMetricId::DiskTemperature | DisplayMetricId::FanSpeed
            ))
            .collect::<Vec<_>>(),
        vec![
            DisplayMetricId::Memory,
            DisplayMetricId::Cpu,
            DisplayMetricId::Network,
            DisplayMetricId::Disk,
            DisplayMetricId::Gpu,
            DisplayMetricId::CpuTemperature,
            DisplayMetricId::GpuTemperature,
            DisplayMetricId::BatteryTemperature,
            DisplayMetricId::ChargingPower,
            DisplayMetricId::BatteryPower,
            DisplayMetricId::SystemPower,
        ]
    );
    assert!(!migrated.shows(MetricId::Gpu));
    assert!(migrated.shows(MetricId::Network));
    let mut selected = migrated;
    selected
        .metrics
        .iter_mut()
        .find(|metric| metric.id == DisplayMetricId::Gpu)
        .unwrap()
        .enabled = true;
    assert_eq!(
        decode_fixture(serde_json::to_value(&selected).unwrap()).unwrap(),
        selected
    );
    let mut future = serde_json::to_value(selected).unwrap();
    future["schemaVersion"] = (PREFERENCE_SCHEMA_VERSION + 1).into();
    assert!(decode_fixture(future).is_err());
}

#[test]
fn old_color_rules_preserve_thresholds_and_do_not_enable_temperature() {
    let mut saved = version_fourteen_preferences();
    saved["usageColors"] = false.into();
    saved["usageWarningPercent"] = 55.into();
    saved["usageCriticalPercent"] = 80.into();
    let restored = decode_fixture(saved).unwrap();
    assert!(!restored.usage_colors);
    assert_eq!(restored.color_rules[0].rules[1].bounds().0, 55);
    assert_eq!(restored.color_rules[0].rules[2].bounds().0, 80);
    assert_eq!(
        restored.color_rules[0].rules[0].color(),
        super::super::color_rules::DisplayColor::Default
    );
    assert!(!restored.color_rules[1].enabled);
    let serialized = serde_json::to_value(&restored).unwrap();
    assert!(serialized.get("usageWarningPercent").is_none());
    assert_eq!(decode_fixture(serialized).unwrap(), restored);
}

fn version_fourteen_preferences() -> serde_json::Value {
    let mut value = serde_json::to_value(ResidentPreferences::default()).unwrap();
    value.as_object_mut().unwrap().remove("colorRules");
    value["schemaVersion"] = 14.into();
    value["usageWarningPercent"] = 70.into();
    value["usageCriticalPercent"] = 90.into();
    value
}

#[test]
fn version_seven_preserves_order_and_adds_display_options() {
    let mut old = legacy_preferences();
    old["metrics"].as_array_mut().unwrap().reverse();
    let mut order = old["metrics"].clone();
    order
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"gpu", "enabled":false}));
    order
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"cpuTemperature", "enabled":false}));
    order
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"gpuTemperature","enabled":false}));
    let migrated = decode_fixture(old).unwrap();
    assert_eq!(migrated.schema_version, 25);
    assert!(!migrated.menu_bar_compact);
    assert!(migrated.usage_colors);
    assert_eq!(
        previous_rows(&serde_json::to_value(migrated).unwrap()["metrics"]),
        previous_rows(&order)
    );
    assert_eq!(
        ResidentPreferences::default()
            .metrics
            .iter()
            .map(|m| m.id)
            .collect::<Vec<_>>(),
        DISPLAY_ORDER
    );
}

#[test]
fn legacy_default_order_moves_network_last_without_changing_enabled_flags() {
    let mut old = legacy_preferences();
    old["metrics"] = serde_json::json!([
        {"id":"cpu", "enabled":false}, {"id":"memory", "enabled":true},
        {"id":"network", "enabled":true}, {"id":"disk", "enabled":false}
    ]);
    let migrated = decode_fixture(old).unwrap();
    assert_eq!(
        migrated
            .metrics
            .iter()
            .map(|m| m.id)
            .filter(|id| !matches!(
                id,
                DisplayMetricId::DiskTemperature | DisplayMetricId::FanSpeed
            ))
            .collect::<Vec<_>>(),
        [
            DisplayMetricId::Cpu,
            DisplayMetricId::Memory,
            DisplayMetricId::Disk,
            DisplayMetricId::Gpu,
            DisplayMetricId::CpuTemperature,
            DisplayMetricId::GpuTemperature,
            DisplayMetricId::BatteryTemperature,
            DisplayMetricId::ChargingPower,
            DisplayMetricId::BatteryPower,
            DisplayMetricId::SystemPower,
            DisplayMetricId::Network,
        ]
    );
    assert!(migrated.shows(MetricId::Network));
    assert!(!migrated.shows(MetricId::Disk));
}

#[test]
fn legacy_thresholds_reject_invalid_ranges() {
    for (warning, critical) in [(0, 90), (90, 90), (95, 90), (70, 101)] {
        let mut value = version_fourteen_preferences();
        value["usageWarningPercent"] = warning.into();
        value["usageCriticalPercent"] = critical.into();
        assert!(decode_fixture(value).is_err());
    }
    let mut malformed = legacy_preferences();
    malformed["usageColors"] = true.into();
    assert!(decode_fixture(malformed).is_err());
}

#[test]
fn version_five_preserves_manual_positions_and_current_auto_round_trips() {
    for position in ["left", "right"] {
        let mut old = legacy_preferences();
        old["schemaVersion"] = 5.into();
        old.as_object_mut()
            .unwrap()
            .remove("windowsCpuTemperatureEnabled");
        old.as_object_mut().unwrap().remove("gpuAdapter");
        old.as_object_mut().unwrap().remove("taskbarCompact");
        old["taskbarPosition"] = position.into();
        let migrated = decode_fixture(old).unwrap();
        assert_eq!(migrated.schema_version, 25);
        assert_eq!(
            serde_json::to_value(migrated).unwrap()["taskbarPosition"],
            position
        );
    }
    let current = ResidentPreferences::default();
    assert_eq!(
        decode_fixture(serde_json::to_value(&current).unwrap()).unwrap(),
        current
    );
}

#[test]
fn version_six_preserves_automatic_position_and_defaults_to_standard_density() {
    let mut old = legacy_preferences();
    old["schemaVersion"] = 6.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    old.as_object_mut().unwrap().remove("gpuAdapter");
    old.as_object_mut().unwrap().remove("taskbarCompact");
    let migrated = decode_fixture(old).unwrap();
    assert_eq!(migrated.taskbar_position, TaskbarPosition::Auto);
    assert!(!migrated.taskbar_compact);
    let mut compact = migrated;
    compact.taskbar_compact = true;
    assert_eq!(
        decode_fixture(serde_json::to_value(&compact).unwrap()).unwrap(),
        compact
    );
}

#[test]
fn fresh_preferences_show_logo_cpu_memory_with_automatic_taskbar_position() {
    let preferences = ResidentPreferences::default();
    assert!(preferences.enabled && preferences.show_icon && preferences.taskbar_background);
    assert_eq!(preferences.taskbar_position, TaskbarPosition::Auto);
    assert_eq!(
        preferences
            .metrics
            .iter()
            .filter(|metric| metric.enabled)
            .map(|metric| metric.id)
            .filter(|id| !matches!(
                id,
                DisplayMetricId::DiskTemperature | DisplayMetricId::FanSpeed
            ))
            .collect::<Vec<_>>(),
        vec![DisplayMetricId::Cpu, DisplayMetricId::Memory]
    );
    assert!(preferences.network_interface.is_none() && preferences.disk_volume.is_none());
}

#[test]
fn saved_preferences_are_not_replaced_by_new_install_defaults() {
    let mut saved = ResidentPreferences {
        taskbar_position: TaskbarPosition::Right,
        enabled: false,
        show_icon: false,
        taskbar_background: false,
        ..Default::default()
    };
    for metric in &mut saved.metrics {
        metric.enabled = matches!(metric.id, DisplayMetricId::Network | DisplayMetricId::Disk);
    }
    assert_eq!(
        decode_fixture(serde_json::to_value(&saved).unwrap()).unwrap(),
        saved
    );
}

#[test]
fn migration_preserves_old_background_and_memory_choices() {
    for enabled in [false, true] {
        for memory in [false, true] {
            let migrated = decode_fixture(
                serde_json::json!({"schemaVersion":1,"enabled":enabled,"showMemory":memory}),
            )
            .unwrap();
            assert_eq!(migrated.taskbar_position, TaskbarPosition::Right);
            assert_eq!(migrated.enabled, enabled);
            assert_eq!(migrated.shows(MetricId::Memory), memory);
            assert!(migrated.show_icon);
            assert!(!migrated.shows(MetricId::Cpu));
            assert_eq!(migrated.schema_version, 25);
        }
    }
}

#[test]
fn version_two_migrates_to_taskbar_without_changing_metric_choices() {
    let mut old = legacy_preferences();
    old["schemaVersion"] = 2.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    old.as_object_mut().unwrap().remove("gpuAdapter");
    old.as_object_mut().unwrap().remove("taskbarCompact");
    old.as_object_mut().unwrap().remove("taskbarBackground");
    old["revision"] = 17.into();
    old["showIcon"] = false.into();
    old.as_object_mut().unwrap().remove("windowsDisplayMode");
    old.as_object_mut().unwrap().remove("taskbarPosition");
    let migrated = decode_fixture(old).unwrap();
    assert_eq!(migrated.revision, 17);
    assert!(!migrated.show_icon);
    assert_eq!(
        decode_fixture(serde_json::to_value(&migrated).unwrap()).unwrap(),
        migrated
    );
}

#[test]
fn version_three_preserves_choices_and_defaults_to_right() {
    let mut old = legacy_preferences();
    old["schemaVersion"] = 3.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    old.as_object_mut().unwrap().remove("gpuAdapter");
    old.as_object_mut().unwrap().remove("taskbarCompact");
    old.as_object_mut().unwrap().remove("taskbarBackground");
    old["windowsDisplayMode"] = "taskbar".into();
    old["revision"] = 42.into();
    old.as_object_mut().unwrap().remove("taskbarPosition");
    let migrated = decode_fixture(old).unwrap();
    assert_eq!(migrated.taskbar_position, TaskbarPosition::Right);
    assert_eq!(migrated.revision, 42);
    let mut current = serde_json::to_value(migrated).unwrap();
    current["taskbarPosition"] = "left".into();
    assert_eq!(
        decode_fixture(current.clone()).unwrap().taskbar_position,
        TaskbarPosition::Left
    );
    current["taskbarPosition"] = "middle".into();
    assert!(decode_fixture(current).is_err());
}

#[test]
fn version_four_keeps_background_and_current_preserves_transparency() {
    let mut old = legacy_preferences();
    old["schemaVersion"] = 4.into();
    old.as_object_mut()
        .unwrap()
        .remove("windowsCpuTemperatureEnabled");
    old.as_object_mut().unwrap().remove("gpuAdapter");
    old.as_object_mut().unwrap().remove("taskbarCompact");
    old["taskbarPosition"] = "left".into();
    old["windowsDisplayMode"] = "taskbar".into();
    old.as_object_mut().unwrap().remove("taskbarBackground");
    let migrated = decode_fixture(old.clone()).unwrap();
    assert!(migrated.taskbar_background);
    assert_eq!(migrated.taskbar_position, TaskbarPosition::Left);
    let mut current = serde_json::to_value(migrated).unwrap();
    current["taskbarBackground"] = false.into();
    assert!(!decode_fixture(current.clone()).unwrap().taskbar_background);
    current["taskbarBackground"] = "false".into();
    assert!(decode_fixture(current).is_err());
    old["taskbarBackground"] = false.into();
    assert!(decode_fixture(old).is_err());
}

#[test]
fn duplicate_ids_preserve_first_order_and_missing_items_are_disabled() {
    let preferences = ResidentPreferences {
        metrics: vec![
            DisplayMetric {
                id: DisplayMetricId::Network,
                enabled: true,
            },
            DisplayMetric {
                id: DisplayMetricId::Network,
                enabled: false,
            },
        ],
        ..Default::default()
    }
    .normalize()
    .unwrap();
    assert_eq!(preferences.metrics.len(), 13);
    assert_eq!(
        preferences.metrics.last().unwrap().id,
        DisplayMetricId::Network
    );
    assert!(preferences.shows(MetricId::Network));
    assert!(!preferences.shows(MetricId::Memory));
}

#[test]
fn unknown_versions_and_invalid_types_are_rejected_without_migration() {
    for value in [
        serde_json::json!({"schemaVersion":99}),
        serde_json::json!({"schemaVersion":1,"enabled":"true","showMemory":false}),
        serde_json::json!({"schemaVersion":1,"enabled":true}),
    ] {
        assert!(decode_fixture(value).is_err());
    }
    let mut value = version_fourteen_preferences();
    value.as_object_mut().unwrap().remove("gpuAdapter");
    value["metrics"][0]["id"] = "npu".into();
    assert!(decode_fixture(value).is_err());
}

#[test]
fn every_display_combination_retains_an_entry() {
    for bits in 0..(1 << DISPLAY_ORDER.len()) {
        for show_icon in [false, true] {
            let mut preferences = ResidentPreferences {
                show_icon,
                ..Default::default()
            };
            for (index, metric) in preferences.metrics.iter_mut().enumerate() {
                metric.enabled = bits & (1 << index) != 0;
            }
            assert_eq!(
                preferences.effective_icon(),
                show_icon
                    || !preferences.metrics.iter().any(|m| {
                        m.enabled
                            && (cfg!(target_os = "macos")
                                || !matches!(m.id, DisplayMetricId::CpuTemperature))
                            && (cfg!(any(target_os = "macos", windows))
                                || !matches!(
                                    m.id,
                                    DisplayMetricId::GpuTemperature
                                        | DisplayMetricId::DiskTemperature
                                ))
                            && (cfg!(target_os = "macos")
                                || !matches!(
                                    m.id,
                                    DisplayMetricId::BatteryTemperature
                                        | DisplayMetricId::ChargingPower
                                        | DisplayMetricId::BatteryPower
                                        | DisplayMetricId::SystemPower
                                        | DisplayMetricId::FanSpeed
                                ))
                    })
            );
            assert!(
                preferences.effective_icon()
                    || preferences.metrics.iter().any(|metric| metric.enabled)
            );
        }
    }
}
