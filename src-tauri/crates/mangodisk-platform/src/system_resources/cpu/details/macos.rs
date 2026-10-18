//! Apple Silicon active-state averages. Unknown tables or channel layouts fail closed.
use super::{CpuFrequency, CpuFrequencySource};
use crate::{PlatformError, PlatformErrorCode};

mod frequency_layout;
use crate::PlatformResult;
use core_foundation::{
    array::CFArray,
    base::{CFType, CFTypeRef, TCFType},
    data::CFData,
    dictionary::{CFDictionary, CFMutableDictionaryRef},
    string::{CFString, CFStringRef},
};
use frequency_layout::{active_average, cluster, decode_table, CoreKind, KINDS};
use std::{
    ffi::c_char,
    ptr,
    time::{Duration, Instant},
};

#[link(name = "IOReport")]
unsafe extern "C" {
    fn IOReportCopyChannelsInGroup(
        group: CFStringRef,
        subgroup: CFStringRef,
        a: u64,
        b: u64,
        c: u64,
    ) -> core_foundation::dictionary::CFDictionaryRef;
    fn IOReportCreateSubscription(
        a: *const std::ffi::c_void,
        channels: CFMutableDictionaryRef,
        out: *mut CFMutableDictionaryRef,
        b: u64,
        c: CFTypeRef,
    ) -> CFTypeRef;
    fn IOReportCreateSamples(
        subscription: CFTypeRef,
        channels: CFMutableDictionaryRef,
        c: CFTypeRef,
    ) -> core_foundation::dictionary::CFDictionaryRef;
    fn IOReportCreateSamplesDelta(
        a: core_foundation::dictionary::CFDictionaryRef,
        b: core_foundation::dictionary::CFDictionaryRef,
        c: CFTypeRef,
    ) -> core_foundation::dictionary::CFDictionaryRef;
    fn IOReportChannelGetChannelName(
        channel: core_foundation::dictionary::CFDictionaryRef,
    ) -> CFStringRef;
    fn IOReportStateGetCount(channel: core_foundation::dictionary::CFDictionaryRef) -> i32;
    fn IOReportStateGetNameForIndex(
        channel: core_foundation::dictionary::CFDictionaryRef,
        index: i32,
    ) -> CFStringRef;
    fn IOReportStateGetResidency(
        channel: core_foundation::dictionary::CFDictionaryRef,
        index: i32,
    ) -> i64;
}
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceNameMatching(name: *const c_char) -> CFMutableDictionaryRef;
    fn IOServiceGetMatchingServices(
        port: u32,
        matching: CFMutableDictionaryRef,
        iterator: *mut u32,
    ) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: CFStringRef,
        allocator: *const std::ffi::c_void,
        options: u32,
    ) -> CFTypeRef;
}
struct Object(u32);
impl Drop for Object {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                IOObjectRelease(self.0);
            }
        }
    }
}
fn unavailable(stage: &str, reason: impl std::fmt::Display) -> PlatformError {
    PlatformError::new(
        PlatformErrorCode::Unsupported,
        format!("source=ioreport_performance_states stage={stage} {reason}"),
    )
}
fn property(object: u32, key: &str) -> Option<CFType> {
    let key = CFString::new(key);
    let raw = unsafe {
        IORegistryEntryCreateCFProperty(object, key.as_concrete_TypeRef(), ptr::null(), 0)
    };
    (!raw.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(raw) })
}
fn table(object: u32, key: &str, divisor: f64) -> PlatformResult<Option<Vec<f64>>> {
    let Some(value) = property(object, key) else {
        return Ok(None);
    };
    let data = value
        .downcast::<CFData>()
        .ok_or_else(|| unavailable("decode_table", format!("key={key} reason=not_data")))?;
    decode_table(data.bytes(), divisor)
        .map(Some)
        .ok_or_else(|| {
            unavailable(
                "decode_table",
                format!(
                    "key={key} bytes={} divisor={divisor} reason=invalid_frequency_table",
                    data.len()
                ),
            )
        })
}
fn load_tables(generation: u32) -> PlatformResult<[Vec<f64>; 3]> {
    let mut tables: [Vec<f64>; 3] = Default::default();
    for node in [c"pmgr", c"pmgr-child"] {
        let mut iterator = 0;
        let code = unsafe {
            IOServiceGetMatchingServices(0, IOServiceNameMatching(node.as_ptr()), &mut iterator)
        };
        if code != 0 {
            return Err(unavailable(
                "match_power_manager",
                format!("node={} native_code={code}", node.to_string_lossy()),
            ));
        }
        let iterator = Object(iterator);
        loop {
            let entry = unsafe { IOIteratorNext(iterator.0) };
            if entry == 0 {
                break;
            }
            let entry = Object(entry);
            let clusters = (generation >= 5)
                .then(|| property(entry.0, "acc-clusters"))
                .flatten();
            let data = clusters
                .as_ref()
                .and_then(|value| value.downcast::<CFData>());
            if clusters.is_some() && data.is_none() {
                return Err(unavailable("decode_clusters", "reason=not_data"));
            }
            let keys =
                frequency_layout::table_keys(generation, data.as_ref().map(|data| data.bytes()))
                    .ok_or_else(|| {
                        unavailable(
                            "decode_clusters",
                            format!(
                        "node={} generation={generation} bytes={} reason=unknown_cluster_layout",
                        node.to_string_lossy(),
                        data.as_ref().map_or(0, |data| data.len()),
                    ),
                        )
                    })?;
            for kind in KINDS {
                let index = kind as usize;
                if tables[index].is_empty() {
                    for slot in keys[index].into_iter().flatten() {
                        let key = format!("voltage-states{slot}-sram");
                        match table(entry.0, &key, frequency_layout::divisor(generation)) {
                            Ok(Some(values)) => {
                                log::info!("cpu_frequency_table node={} kind={kind:?} key={key} states={} min_mhz={} max_mhz={}", node.to_string_lossy(), values.len(), values.iter().copied().fold(f64::INFINITY, f64::min), values.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                                tables[index] = values;
                                break;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                // An absent core tier may retain an empty firmware table. Only
                                // identified live channels require a table, checked in read().
                                log::info!("cpu_frequency_table_rejected node={} kind={kind:?} key={key} code={:?} error={} outcome=try_next_identified_table",node.to_string_lossy(),error.code(),crate::diagnostics::text(&error));
                            }
                        }
                    }
                }
            }
        }
    }
    if frequency_layout::complete_tables(generation, &mut tables) {
        log::info!(
            "cpu_frequency_table kind=Performance source=shared_pacc generation=6 states={}",
            tables[CoreKind::Performance as usize].len()
        );
    }
    if tables.iter().all(Vec::is_empty) {
        return Err(unavailable(
            "load_tables",
            format!("generation={generation} nodes=pmgr,pmgr-child reason=no_identified_tables"),
        ));
    }
    Ok(tables)
}
struct Subscription {
    channels: CFType,
    subscription: CFType,
    previous: Option<(CFType, Instant)>,
    generation: u32,
    tables: [Vec<f64>; 3],
}
fn dictionary(value: &CFType) -> Option<CFDictionary<CFString, CFType>> {
    let value = value.downcast::<CFDictionary>()?;
    Some(unsafe { CFDictionary::wrap_under_get_rule(value.as_concrete_TypeRef()) })
}
impl Subscription {
    fn open() -> PlatformResult<Self> {
        let model = super::identity()
            .model
            .ok_or_else(|| unavailable("identify_model", "reason=model_missing"))?;
        let generation = frequency_layout::generation(&model).ok_or_else(|| {
            unavailable(
                "identify_model",
                format!(
                    "model={} reason=unidentified_generation",
                    crate::diagnostics::text(&model)
                ),
            )
        })?;
        let tables = load_tables(generation)?;
        let group = CFString::new("CPU Stats");
        // Newer chips expose EACC/PACC complex names; their per-core channels retain the
        // ECPU/MCPU/PCPU identity needed to keep three distinct frequency classes.
        let subgroup = CFString::new(if generation >= 5 {
            "CPU Core Performance States"
        } else {
            "CPU Complex Performance States"
        });
        let raw = unsafe {
            IOReportCopyChannelsInGroup(
                group.as_concrete_TypeRef(),
                subgroup.as_concrete_TypeRef(),
                0,
                0,
                0,
            )
        };
        if raw.is_null() {
            return Err(unavailable(
                "create_channels",
                format!(
                    "model={} subgroup={} reason=null_result",
                    crate::diagnostics::text(&model),
                    subgroup
                ),
            ));
        }
        let source = unsafe { CFType::wrap_under_create_rule(raw.cast()) };
        let channels = unsafe {
            core_foundation::dictionary::CFDictionaryCreateMutableCopy(
                ptr::null(),
                0,
                source.as_CFTypeRef().cast(),
            )
        };
        if channels.is_null() {
            return Err(unavailable("copy_channels", "reason=null_result"));
        }
        let channels = unsafe { CFType::wrap_under_create_rule(channels.cast()) };
        let mut output = ptr::null_mut();
        let raw = unsafe {
            IOReportCreateSubscription(
                ptr::null(),
                channels.as_CFTypeRef().cast_mut().cast(),
                &mut output,
                0,
                ptr::null(),
            )
        };
        if !output.is_null() {
            drop(unsafe { CFType::wrap_under_create_rule(output.cast()) });
        }
        if raw.is_null() {
            return Err(unavailable("subscribe", "reason=null_result"));
        }
        let subscription = unsafe { CFType::wrap_under_create_rule(raw) };
        log::info!("cpu_frequency_capability model={} generation={generation} subgroup={} efficiency_states={} performance_states={} super_states={}",crate::diagnostics::text(&model),subgroup,tables[0].len(),tables[1].len(),tables[2].len());
        Ok(Self {
            channels,
            subscription,
            previous: None,
            generation,
            tables,
        })
    }
    fn read(&mut self) -> PlatformResult<Option<CpuFrequency>> {
        let raw = unsafe {
            IOReportCreateSamples(
                self.subscription.as_CFTypeRef(),
                self.channels.as_CFTypeRef().cast_mut().cast(),
                ptr::null(),
            )
        };
        if raw.is_null() {
            return Err(unavailable("sample", "reason=null_result"));
        }
        let current = unsafe { CFType::wrap_under_create_rule(raw.cast()) };
        let now = Instant::now();
        let Some((previous, at)) = self.previous.replace((current.clone(), now)) else {
            return Ok(None);
        };
        if !(100..=5000).contains(&now.duration_since(at).as_millis()) {
            return Ok(None);
        }
        let raw = unsafe {
            IOReportCreateSamplesDelta(
                previous.as_CFTypeRef().cast(),
                current.as_CFTypeRef().cast(),
                ptr::null(),
            )
        };
        if raw.is_null() {
            return Err(unavailable("sample_delta", "reason=null_result"));
        }
        let delta = unsafe { CFType::wrap_under_create_rule(raw.cast()) };
        let dictionary = dictionary(&delta)
            .ok_or_else(|| unavailable("decode_sample", "reason=not_dictionary"))?;
        let list = dictionary
            .find(CFString::new("IOReportChannels"))
            .and_then(|value| value.downcast::<CFArray>())
            .ok_or_else(|| unavailable("decode_channels", "reason=channels_missing_or_invalid"))?;
        let mut values: [Vec<f64>; 3] = Default::default();
        let mut matched = 0;
        let mut observed_names = Vec::new();
        for index in 0..list.len() {
            let item = unsafe {
                core_foundation::array::CFArrayGetValueAtIndex(list.as_concrete_TypeRef(), index)
            };
            if item.is_null()
                || unsafe { core_foundation::base::CFGetTypeID(item) }
                    != CFDictionary::<CFType, CFType>::type_id()
            {
                return Err(unavailable(
                    "decode_channels",
                    format!("index={index} reason=not_dictionary"),
                ));
            }
            let item = item.cast();
            let name = unsafe { IOReportChannelGetChannelName(item) };
            if name.is_null() {
                continue;
            }
            let name = unsafe { CFString::wrap_under_get_rule(name) }.to_string();
            let Some(kind) = cluster(&name, self.generation) else {
                if observed_names.len() < 24 {
                    observed_names.push(crate::diagnostics::text(&name));
                }
                continue;
            };
            matched += 1;
            let count = unsafe { IOReportStateGetCount(item) };
            if !(1..=68).contains(&count) {
                return Err(unavailable(
                    "decode_states",
                    format!(
                        "channel={} count={count} reason=invalid_state_count",
                        crate::diagnostics::text(&name)
                    ),
                ));
            }
            let mut states = Vec::new();
            for state in 0..count {
                let state_name = unsafe { IOReportStateGetNameForIndex(item, state) };
                if state_name.is_null() {
                    return Err(unavailable(
                        "decode_states",
                        format!(
                            "channel={} state={state} reason=state_name_missing",
                            crate::diagnostics::text(&name)
                        ),
                    ));
                }
                states.push((
                    unsafe { CFString::wrap_under_get_rule(state_name) }.to_string(),
                    unsafe { IOReportStateGetResidency(item, state) },
                ));
            }
            let frequencies = &self.tables[kind as usize];
            let value = active_average(&states, frequencies).map_err(|reason| {
                unavailable(
                    "decode_states",
                    format!(
                        "channel={} kind={kind:?} count={count} table_states={} state_names={:?} reason={reason}",
                        crate::diagnostics::text(&name),
                        frequencies.len(), states.iter().map(|(name, _)| name.as_str()).take(24).collect::<Vec<_>>()
                    ),
                )
            })?;
            if let Some(value) = value {
                values[kind as usize].push(value);
            }
        }
        if matched == 0 {
            return Err(unavailable("decode_channels", format!("generation={} channel_count={} observed={observed_names:?} reason=no_identified_cpu_channels",self.generation,list.len())));
        }
        fn mean(values: &[f64]) -> Option<f64> {
            (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
        }
        let efficiency_mhz = mean(&values[0]);
        let performance_mhz = mean(&values[1]);
        // Do not substitute the minimum clock when a cluster has no active-state residency.
        let super_mhz = mean(&values[2]);
        if efficiency_mhz.is_none() && performance_mhz.is_none() && super_mhz.is_none() {
            return Ok(None);
        }
        Ok(Some(CpuFrequency {
            // Cluster activity differs; do not invent an overall core-weighted frequency.
            average_mhz: None,
            efficiency_mhz,
            performance_mhz,
            super_mhz,
            source: CpuFrequencySource::ApplePerformanceStates,
        }))
    }
}
#[derive(Default)]
pub(super) struct FrequencyReader {
    subscription: Option<Subscription>,
    retry_at: Option<Instant>,
    previous_error: Option<String>,
}
impl FrequencyReader {
    pub(super) fn pause(&mut self) {
        if let Some(subscription) = &mut self.subscription {
            subscription.previous = None;
        }
    }
    pub(super) fn read(&mut self) -> PlatformResult<Option<CpuFrequency>> {
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            return Err(unavailable("retry_backoff", "retry_seconds=30"));
        }
        let result = (|| {
            if self.subscription.is_none() {
                self.subscription = Some(Subscription::open()?);
            }
            self.subscription
                .as_mut()
                .expect("opened frequency subscription")
                .read()
        })();
        if let Err(error) = &result {
            if self.previous_error.as_deref() != Some(error.diagnostic()) {
                log::info!("cpu_frequency_unavailable source=ioreport_performance_states model={} code={:?} error={} retry_seconds=30",crate::diagnostics::text(super::identity().model.as_deref().unwrap_or("unknown")),error.code(),crate::diagnostics::text(error));
                self.previous_error = Some(error.diagnostic().to_string());
            }
            self.subscription = None;
            self.retry_at = Some(Instant::now() + Duration::from_secs(30));
        }
        if matches!(result, Ok(Some(_))) && self.previous_error.take().is_some() {
            log::info!("cpu_frequency_recovered source=ioreport_performance_states");
        }
        result
    }
}
