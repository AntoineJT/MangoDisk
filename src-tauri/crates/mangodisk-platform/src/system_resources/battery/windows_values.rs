//! Native battery units are converted before any aggregation or presentation.
use super::{
    BatteryPower, BatteryPowerState, BatterySample, BatterySource, BatteryTemperature,
    ChargingPower,
};
use crate::{PlatformError, PlatformErrorCode, PlatformResult};

pub(super) struct BatteryData {
    pub temperature: Option<u32>,
    pub capabilities: u32,
    pub power_state: Option<u32>,
    pub rate: Option<i32>,
}

pub(super) fn sample(batteries: &[BatteryData]) -> PlatformResult<BatterySample> {
    if batteries.is_empty() {
        return Err(PlatformError::new(
            PlatformErrorCode::Unsupported,
            "battery stage=sample reason=no_system_battery",
        ));
    }
    let source = BatterySource::WindowsBattery;
    let temperature = batteries
        .iter()
        .filter_map(|battery| battery.temperature)
        .map(|raw| f64::from(raw) / 10.0 - 273.15)
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 100.0)
        .reduce(f64::max)
        .map(|celsius| BatteryTemperature { celsius, source });
    // Relative capacities use device-specific units, so their rate is not milliwatts.
    // Missing rates never become a plausible idle or zero-power observation.
    let mut signed_watts = Some(0.0);
    let mut external = Some(true);
    for battery in batteries {
        external = external
            .zip(battery.power_state)
            .map(|(external, state)| external && state & 1 != 0);
        let rate = battery.rate.filter(|rate| *rate != i32::MIN);
        let (Some(rate), Some(state)) = (rate, battery.power_state) else {
            signed_watts = None;
            continue;
        };
        if battery.capabilities & 0x4000_0000 != 0 {
            signed_watts = None;
            continue;
        }
        if (rate > 0 && state & 7 != 5)
            || (rate < 0 && state & 6 != 2)
            || state & 6 == 6
            || rate.unsigned_abs() > 300_000
        {
            return Err(PlatformError::new(PlatformErrorCode::InvalidData,
                format!("battery stage=decode_status native_state={state:#x} native_rate_mw={rate} outcome=invalid_direction_or_range")));
        }
        signed_watts = signed_watts.map(|sum| sum + f64::from(rate) / 1000.0);
    }
    let signed_watts = signed_watts.filter(|watts| watts.abs() <= 300.0);
    let battery_power = signed_watts.map(|watts| BatteryPower {
        watts: watts.abs(),
        state: if watts > 0.0 {
            BatteryPowerState::Charging
        } else if watts < 0.0 {
            BatteryPowerState::Discharging
        } else {
            BatteryPowerState::Idle
        },
        source,
    });
    let charging_power = signed_watts
        .zip(external)
        .filter(|(watts, external)| *watts <= 0.0 || *external)
        .map(|(watts, external_connected)| ChargingPower {
            watts: watts.max(0.0),
            charging: watts > 0.0,
            external_connected,
            source,
        });
    Ok(BatterySample {
        temperature,
        battery_power,
        charging_power,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn battery(rate: i32, state: u32) -> BatteryData {
        BatteryData {
            temperature: Some(2982),
            capabilities: 0x8000_0000,
            power_state: Some(state),
            rate: Some(rate),
        }
    }
    #[test]
    fn native_units_and_charge_direction_are_preserved() {
        let value = sample(&[battery(-24_000, 2)]).unwrap();
        assert!((value.temperature.unwrap().celsius - 25.05).abs() < 0.001);
        assert_eq!(value.battery_power.as_ref().unwrap().watts, 24.0);
        assert_eq!(
            value.battery_power.unwrap().state,
            BatteryPowerState::Discharging
        );
        assert_eq!(value.charging_power.unwrap().watts, 0.0);
        assert_eq!(
            sample(&[battery(50_000, 5)])
                .unwrap()
                .charging_power
                .unwrap()
                .watts,
            50.0
        );
        assert_eq!(
            sample(&[battery(0, 1)])
                .unwrap()
                .battery_power
                .unwrap()
                .state,
            BatteryPowerState::Idle
        );
    }
    #[test]
    fn unknown_relative_and_contradictory_rates_are_not_zero_watts() {
        assert!(sample(&[battery(i32::MIN, 2)])
            .unwrap()
            .battery_power
            .is_none());
        for value in [
            battery(-10_000, 4),
            battery(10_000, 2),
            battery(0, 6),
            battery(10_000, 4),
            battery(-10_000, 1),
            battery(301_000, 5),
        ] {
            assert!(sample(&[value]).is_err());
        }
        let mut relative = battery(10_000, 5);
        relative.capabilities |= 0x4000_0000;
        assert!(sample(&[relative]).unwrap().charging_power.is_none());
        let mut value = battery(10_000, 5);
        value.rate = None;
        assert!(sample(&[value]).unwrap().charging_power.is_none());
        assert!(sample(&[]).is_err());
    }
    #[test]
    fn multiple_batteries_require_complete_rates_and_keep_the_hottest_sensor() {
        let mut one = battery(-10_000, 2);
        one.temperature = Some(3032);
        let value = sample(&[one, battery(-20_000, 2)]).unwrap();
        assert_eq!(value.battery_power.unwrap().watts, 30.0);
        assert!((value.temperature.unwrap().celsius - 30.05).abs() < 0.001);
        let mut missing = battery(-20_000, 2);
        missing.rate = None;
        assert!(sample(&[battery(-10_000, 2), missing])
            .unwrap()
            .battery_power
            .is_none());
    }
}
