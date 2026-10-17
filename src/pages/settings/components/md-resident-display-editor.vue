<script setup lang="ts">
import MdResidentMetricOrder from './md-resident-metric-order.vue';
import MdTooltip from '@/components/custom/md-tooltip.vue';
import { METRIC_LABEL_KEYS, METRIC_STATUS_KEYS } from '@/lib/models/system-resources/system-resources';
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import MdCheckbox from '@/components/custom/md-checkbox.vue';
import MdIcon from '@/components/icons/md-icon.vue';
import MdIconMangodisk from '@/components/icons/md-icon-mangodisk.vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogTitle } from '@/components/ui/dialog';
import MdDialogContent from '@/components/custom/md-dialog-content.vue';
import MdDialogHeader from '@/components/custom/md-dialog-header.vue';
import MdDialogFooter from '@/components/custom/md-dialog-footer.vue';
import MdWindowsTemperatureSetup from './md-windows-temperature-setup.vue';
import { Select, SelectContent, SelectItem } from '@/components/ui/select';
import { SelectTrigger } from 'reka-ui';
import MdTaskbarPosition from './md-taskbar-position.vue';
import MdStatusAppearance from './md-status-appearance.vue';
import MdFanDisplaySettings from './md-fan-display-settings.vue';
import MdStatusColorRuleEditor from './md-status-color-rule-editor.vue';
import type { ColorMetricId, MetricColorRule } from '@/lib/models/resident/resident-color-rules';
import { ICON_NAMES } from '@/lib/models/presentation/ui';
import type { NetworkInterface, ResourceVolume } from '@/lib/models/system-resources/system-resources';
import type { ResidentPreferences, ResidentReading, ResidentDisplayMetricId } from '@/lib/models/resident/resident';
import { ResidentDisplayCatalogService } from '@/lib/services/resident/resident-display-catalog-service';
import { useResidentSettingsStore } from '@/stores/resident-settings-store';
import { useWindowsMonitoringStore } from '@/stores/windows-monitoring-store';
import { currentGpuTemperature } from '@/lib/utils/system-resources/gpu-temperature';

const props = withDefaults(defineProps<{ isMacOs: boolean; isLinux?: boolean }>(), {
  isLinux: false,
});
const { t } = useI18n({ useScope: 'global' });
const settings = useResidentSettingsStore();
const monitoring = useWindowsMonitoringStore();
const cpuTemperatureStatusLabel = computed(() => {
  const key = monitoring.statusLabel(settings.preferences?.windowsCpuTemperatureEnabled ?? false);
  return key === 'windowsMonitoring.states.unsupportedHardware' ? 'systemStatus.unsupported' : key;
});
const temperatureSelectionPending = ref(false);
const interfaces = ref<NetworkInterface[]>([]);
const volumes = ref<ResourceVolume[]>([]);
const gpuAdapters = ref<{ id: string; name: string }[]>([]);
const catalogueError = ref(false);
const gpuTemperatureReading = ref<ResidentReading['gpuTemperature']>({
  status: 'loading',
  sampledAtMs: null,
  value: null,
});
const gpuTemperatureObservedAtMs = ref(0);
const gpuSelectionObservedAtMs = ref<number | null>(null);
watch(
  () => [settings.draft?.gpuAdapter, settings.preferences?.gpuAdapter] as const,
  (selection, previous) => {
    if (selection.some((adapter, index) => previous[index] !== undefined && adapter !== previous[index])) {
      // Other metrics can publish newer snapshots containing the previous GPU
      // sample. Wait for a temperature sample taken after the selection change.
      gpuSelectionObservedAtMs.value = gpuTemperatureObservedAtMs.value;
    }
  },
  { flush: 'sync' }
);
const gpuTemperatureAvailable = computed(() => {
  if (settings.draft?.gpuAdapter !== settings.preferences?.gpuAdapter) return false;
  if (
    gpuSelectionObservedAtMs.value !== null &&
    (gpuTemperatureReading.value.sampledAtMs ?? 0) <= gpuSelectionObservedAtMs.value
  ) {
    return false;
  }
  const value = currentGpuTemperature(gpuTemperatureReading.value, gpuTemperatureObservedAtMs.value);
  return value !== null && (!settings.draft?.gpuAdapter || value.adapterId === settings.draft.gpuAdapter);
});
const gpuTemperatureStatus = computed(() =>
  gpuTemperatureReading.value.status === 'ready' && !gpuTemperatureAvailable.value
    ? 'loading'
    : gpuTemperatureReading.value.status
);
const batteryPresent = ref<boolean | null>(null);
const batteryTemperatureStatus = ref<ResidentReading['batteryTemperature']['status']>('loading');
const chargingPowerStatus = ref<ResidentReading['chargingPower']['status']>('loading');
const systemPowerStatus = ref<ResidentReading['systemPower']['status']>('loading');
const fanReading = ref<ResidentReading['fanSpeed']>({ status: 'loading', sampledAtMs: null, value: null });
const fans = computed(() => fanReading.value.value?.fans ?? []);
const batteryPowerStatus = ref<ResidentReading['batteryPower']['status']>('loading');
function batteryHint(id: ResidentDisplayMetricId) {
  const unavailable =
    id === 'batteryTemperature'
      ? batteryTemperatureStatus.value === 'unsupported'
      : id === 'batteryPower'
        ? batteryPowerStatus.value === 'unsupported'
        : chargingPowerStatus.value === 'unsupported';
  return t(
    unavailable
      ? 'systemStatus.batteryUnsupportedHint'
      : id === 'batteryPower'
        ? 'systemStatus.batteryPowerHint'
        : 'systemStatus.chargingPowerHint'
  );
}
const metricOrder = ref<InstanceType<typeof MdResidentMetricOrder> | null>(null);
const dragging = computed(() => metricOrder.value?.dragging ?? null);
const dragRows = ref<ResidentPreferences['metrics'] | null>(null);
function cancelDrag() {
  metricOrder.value?.cancelDrag();
}
const rows = computed(() => dragRows.value ?? settings.draft?.metrics ?? []);
function isBatteryMetric(id: ResidentDisplayMetricId | null): boolean {
  return id === 'batteryTemperature' || id === 'chargingPower' || id === 'batteryPower';
}
const displayLabelKeys = {
  ...METRIC_LABEL_KEYS,
  cpuTemperature: 'cpuTemperature.label',
  gpuTemperature: 'gpuTemperature.label',
  diskTemperature: 'systemStatus.diskTemperature',
  batteryTemperature: 'systemStatus.batteryTemperature',
  chargingPower: 'systemStatus.chargingPower',
  batteryPower: 'systemStatus.batteryPower',
  systemPower: 'systemStatus.systemPower',
  fanSpeed: 'systemStatus.fanSpeed',
};
const visibleRows = computed(() =>
  rows.value
    .filter(
      row =>
        (batteryPresent.value !== false || !isBatteryMetric(row.id)) &&
        (!props.isLinux || row.id !== 'gpuTemperature') &&
        (!props.isLinux || row.id !== 'cpuTemperature') &&
        (!props.isLinux || row.id !== 'diskTemperature') &&
        (!props.isLinux || row.id !== 'gpu') &&
        (props.isMacOs || (!isBatteryMetric(row.id) && row.id !== 'systemPower' && row.id !== 'fanSpeed'))
    )
    .map(row =>
      row.id === 'cpuTemperature' && !props.isMacOs && !settings.draft?.windowsCpuTemperatureEnabled
        ? { ...row, enabled: false }
        : row
    )
);
// Older preferences may have every item cleared. Mirror the native Logo
// fallback without writing on load, and retain it when the next metric is added.
const showIcon = computed(() => (settings.draft?.showIcon ?? true) || !visibleRows.value.some(row => row.enabled));
const selectedCount = computed(() => Number(showIcon.value) + visibleRows.value.filter(row => row.enabled).length);
function selectionLocked(selected: boolean) {
  return !settings.draft || (selected && selectedCount.value <= 1);
}
function enableIcon(enabled: boolean) {
  if (!enabled && selectionLocked(showIcon.value)) return;
  void settings.change({ showIcon: enabled });
}
const settingsOpen = defineModel<boolean>('open', { required: true });
const temperatureSetupOpen = ref(false);
const selectTemperatureAfterSetup = ref(false);
const activeColor = ref<ColorMetricId | null>(null);
watch(batteryPresent, present => {
  if (present !== false) return;
  if (isBatteryMetric(activeColor.value)) activeColor.value = null;
  if (isBatteryMetric(dragging.value)) cancelDrag();
});
function openColor(id: ColorMetricId) {
  cancelDrag();
  expandedSelection.value = null;
  activeColor.value = id;
}
async function saveColor(rule: MetricColorRule) {
  if (!settings.draft) return;
  await settings.change({ colorRules: settings.draft.colorRules.map(saved => (saved.id === rule.id ? rule : saved)) });
  if (!settings.error) activeColor.value = null;
}
const expandedSelection = ref<'network' | 'disk' | 'gpu' | 'gpuTemperature' | 'diskTemperature' | 'fanSpeed' | null>(
  null
);
// Keep the native switch and collapsed content on the committed state. Failed
// writes must not hide the user's controls or leave a misleading enabled state.
const displayEnabled = computed(() => settings.preferences?.enabled ?? false);
watch(displayEnabled, enabled => {
  if (!enabled) {
    settingsOpen.value = false;
    cancelDrag();
    expandedSelection.value = null;
    activeColor.value = null;
  }
});
watch(settingsOpen, open => {
  if (open) {
    // The detail panel can change the shared GPU choice in another WebView.
    void settings.load();
    void loadCatalogue();
    if (!props.isMacOs && !props.isLinux) void monitoring.load();
  } else {
    // A dismissed editor must not leave a floating drag preview or a pending
    // reorder that could be committed after reopening. Saved choices stay intact.
    cancelDrag();
    expandedSelection.value = null;
    activeColor.value = null;
  }
});
function handleDialogEscape(event: KeyboardEvent) {
  if (dragging.value) {
    event.preventDefault();
    cancelDrag();
  }
}
function restoreConfigurationFocus(event: Event) {
  if (disposed) return;
  event.preventDefault();
  document.getElementById(displayEnabled.value ? 'resident-configure' : 'resident-enabled')?.focus();
}
function setSelectionOpen(id: ResidentDisplayMetricId, open: boolean) {
  if (
    id !== 'network' &&
    id !== 'disk' &&
    id !== 'gpu' &&
    id !== 'gpuTemperature' &&
    id !== 'diskTemperature' &&
    id !== 'fanSpeed'
  )
    return;
  if (open) {
    expandedSelection.value = id;
  } else if (expandedSelection.value === id) expandedSelection.value = null;
}
watch(rows, value => {
  if (expandedSelection.value && !value.some(row => row.id === expandedSelection.value && row.enabled)) {
    expandedSelection.value = null;
  }
});
const missingInterface = computed(
  () => settings.draft?.networkInterface && !interfaces.value.some(item => item.id === settings.draft?.networkInterface)
);
const missingVolume = computed(
  () => settings.draft?.diskVolume && !volumes.value.some(item => item.id === settings.draft?.diskVolume)
);
const missingGpu = computed(
  () => settings.draft?.gpuAdapter && !gpuAdapters.value.some(item => item.id === settings.draft?.gpuAdapter)
);
const selectedGpuLabel = computed(() => {
  const id = settings.draft?.gpuAdapter;
  if (!id) return t('systemStatus.automatic');
  return gpuAdapters.value.find(item => item.id === id)?.name ?? t('systemStatus.savedDisconnected');
});
// Select's inferred item text can outlive a locale or catalogue update while
// its popup is closed. Keep the displayed selection reactive without reopening it.
const selectedInterfaceLabel = computed(() => {
  const id = settings.draft?.networkInterface;
  if (!id) return t('systemStatus.automatic');
  const item = interfaces.value.find(item => item.id === id);
  if (!item) return t('systemStatus.savedDisconnected');
  return `${item.name}${item.connected ? '' : ` · ${t('systemStatus.disconnected')}`}`;
});
const selectedVolumeLabel = computed(() => {
  const id = settings.draft?.diskVolume;
  if (!id) return t('systemStatus.systemDisk');
  return volumes.value.find(item => item.id === id)?.name ?? t('systemStatus.savedDisconnected');
});
let disposed = false;
function accept(value: ResidentReading) {
  interfaces.value = value.interfaces;
  volumes.value = value.volumes;
  gpuAdapters.value = value.gpuAdapters;
  gpuTemperatureReading.value = value.gpuTemperature;
  gpuTemperatureObservedAtMs.value = value.observedAtMs;
  batteryPresent.value = value.batteryPresent;
  batteryTemperatureStatus.value = value.batteryTemperature.status;
  chargingPowerStatus.value = value.chargingPower.status;
  batteryPowerStatus.value = value.batteryPower.status;
  systemPowerStatus.value = value.systemPower.status;
  fanReading.value = value.fanSpeed;
}
async function enable(id: ResidentDisplayMetricId, enabled: boolean) {
  if (id === 'gpuTemperature' && enabled && !gpuTemperatureAvailable.value) return;
  if (id === 'cpuTemperature' && !props.isMacOs && !props.isLinux && enabled) {
    if (temperatureSelectionPending.value || monitoring.loading || monitoring.busy) return;
    temperatureSelectionPending.value = true;
    try {
      await monitoring.load();
      if (disposed || !settingsOpen.value || !displayEnabled.value) return;
      if (monitoring.loading || monitoring.busy) return;
      if (monitoring.observation?.state !== 'ready' || !settings.preferences?.windowsCpuTemperatureEnabled) {
        selectTemperatureAfterSetup.value = true;
        temperatureSetupOpen.value = true;
        return;
      }
    } finally {
      temperatureSelectionPending.value = false;
    }
  }
  if (!enabled && selectionLocked(rows.value.some(row => row.id === id && row.enabled))) return;
  cancelDrag();
  void settings.change({
    showIcon: showIcon.value,
    metrics: rows.value.map(metric => (metric.id === id ? { ...metric, enabled } : { ...metric })),
  });
}
const catalogue = new ResidentDisplayCatalogService(accept, failed => {
  catalogueError.value = failed;
});
function loadCatalogue() {
  return catalogue.load();
}
onBeforeUnmount(() => {
  disposed = true;
  cancelDrag();
  catalogue.dispose();
});

defineExpose({
  resetInteractions() {
    cancelDrag();
    expandedSelection.value = null;
  },
});
</script>
<template>
  <Dialog :open="settingsOpen && displayEnabled" @update:open="settingsOpen = $event">
    <MdDialogContent
      class="flex min-h-0 flex-col"
      :aria-describedby="undefined"
      @interact-outside.prevent
      @escape-key-down="handleDialogEscape"
      @close-auto-focus="restoreConfigurationFocus"
    >
      <MdDialogHeader class="flex-none border-b border-border/70">
        <DialogTitle>{{
          t(isMacOs ? 'systemStatus.menuBarSettingsTitle' : 'systemStatus.displaySettingsTitle')
        }}</DialogTitle>
      </MdDialogHeader>
      <div class="min-h-0 overflow-y-auto p-5">
        <div
          id="resident-display-options"
          class="display-options"
          :inert="settings.loading"
          :aria-busy="settings.loading"
        >
          <MdTaskbarPosition
            v-if="!isMacOs && !isLinux && settings.draft"
            :preferences="settings.draft"
            @position="settings.change({ taskbarPosition: $event })"
          />
          <MdStatusAppearance
            v-if="settings.draft"
            :preferences="settings.draft"
            :is-mac-os="isMacOs"
            :is-linux="isLinux"
            @change="settings.change($event)"
          />
          <div class="status-controls">
            <div class="controls-heading">
              <h3>{{ t('systemStatus.displayItems') }}</h3>
              <span>{{ t('systemStatus.dragToReorder') }}</span>
            </div>
            <MdResidentMetricOrder
              ref="metricOrder"
              :metrics="settings.draft?.metrics"
              :visible-rows="visibleRows"
              :label-keys="displayLabelKeys"
              @preview="dragRows = $event"
              @commit="settings.change({ metrics: $event })"
              @pointer-start="expandedSelection = null"
            >
              <template #logo>
                <div key="logo" class="status-item">
                  <span class="logo-marker" aria-hidden="true">
                    <MdIconMangodisk :size="18" />
                  </span>
                  <label class="logo-label" for="status-app-icon">
                    <MdCheckbox
                      id="status-app-icon"
                      :model-value="showIcon"
                      :disabled="selectionLocked(showIcon)"
                      @update:model-value="enableIcon($event === true)"
                    />
                    {{ t('systemStatus.showIcon') }}
                  </label>
                </div>
              </template>
              <template #default="{ row, dragging: orderedMetric, pointerDragging, pointerDown, handleKey }">
                <div
                  :key="row.id"
                  class="status-item metric-row"
                  :class="{
                    moving: orderedMetric === row.id,
                    'pointer-moving': pointerDragging && orderedMetric === row.id,
                  }"
                  :data-metric="row.id"
                >
                  <div class="metric-heading">
                    <button
                      class="drag-handle"
                      :disabled="activeColor !== null"
                      :aria-label="t('systemStatus.reorder', { name: t(displayLabelKeys[row.id]) })"
                      :aria-pressed="orderedMetric === row.id"
                      @pointerdown="pointerDown($event, row.id)"
                      @dragstart.prevent
                      @keydown="handleKey($event, row.id)"
                    >
                      <MdIcon :name="ICON_NAMES.grip" :size="15" />
                    </button>
                    <div class="metric-label-group">
                      <label :for="`status-${row.id}`">
                        <MdCheckbox
                          :id="`status-${row.id}`"
                          :model-value="row.enabled"
                          :disabled="
                            selectionLocked(row.enabled) ||
                            (row.id === 'gpuTemperature' && !row.enabled && !gpuTemperatureAvailable) ||
                            (row.id === 'cpuTemperature' &&
                              !isMacOs &&
                              !isLinux &&
                              (temperatureSelectionPending || monitoring.loading || monitoring.busy))
                          "
                          @update:model-value="enable(row.id, $event === true)"
                        />
                        {{
                          row.id === 'cpu'
                            ? t('systemStatus.cpuShort')
                            : row.id === 'disk'
                              ? t('systemStatus.volume')
                              : t(displayLabelKeys[row.id])
                        }}
                      </label>
                      <MdTooltip
                        v-if="
                          row.id === 'chargingPower' ||
                          row.id === 'batteryPower' ||
                          (row.id === 'batteryTemperature' && batteryTemperatureStatus === 'unsupported')
                        "
                        :text="batteryHint(row.id)"
                      >
                        <button type="button" class="md-help-action" :aria-label="batteryHint(row.id)">
                          <MdIcon :name="ICON_NAMES.info" :size="14" />
                        </button>
                      </MdTooltip>
                      <MdTooltip
                        v-if="row.id === 'systemPower' && systemPowerStatus === 'unsupported'"
                        :text="t('systemStatus.systemPowerUnsupportedHint')"
                      >
                        <button
                          type="button"
                          class="md-help-action"
                          :aria-label="t('systemStatus.systemPowerUnsupportedHint')"
                        >
                          <MdIcon :name="ICON_NAMES.info" :size="14" />
                        </button>
                      </MdTooltip>
                      <span
                        v-if="
                          row.id === 'fanSpeed' && row.enabled && (fanReading.status !== 'ready' || fans.length === 0)
                        "
                        role="status"
                        class="text-content-meta text-muted-foreground"
                      >
                        {{
                          t(
                            fanReading.status === 'ready' && fans.length === 0
                              ? 'systemStatus.noFans'
                              : METRIC_STATUS_KEYS[fanReading.status]
                          )
                        }}
                      </span>
                      <span
                        v-if="row.id === 'gpuTemperature' && !gpuTemperatureAvailable"
                        role="status"
                        class="inline-flex shrink-0 items-center rounded bg-muted px-1.5 py-0.5 text-content-meta font-medium leading-4 text-muted-foreground"
                        >{{ t(METRIC_STATUS_KEYS[gpuTemperatureStatus]) }}</span
                      >
                      <span
                        v-if="row.id === 'cpuTemperature' && !isMacOs && !isLinux && cpuTemperatureStatusLabel"
                        role="status"
                        class="inline-flex shrink-0 items-center rounded bg-muted px-1.5 py-0.5 text-content-meta font-medium leading-4"
                        :class="monitoring.needsAttention ? 'text-destructive-text' : 'text-muted-foreground'"
                        >{{ t(cpuTemperatureStatusLabel ?? 'windowsMonitoring.states.notEnabled') }}</span
                      >
                    </div>

                    <Button
                      v-if="row.id === 'cpuTemperature' && !isMacOs && !isLinux"
                      variant="ghost"
                      size="sm"
                      class="text-muted-foreground"
                      @click="
                        selectTemperatureAfterSetup = false;
                        temperatureSetupOpen = true;
                      "
                      >{{ t('windowsMonitoring.manage') }}</Button
                    >
                    <MdFanDisplaySettings
                      v-if="row.id === 'fanSpeed' && settings.draft"
                      :mode="settings.draft.fanDisplay"
                      :ids="settings.draft.fanIds"
                      :reading="fanReading"
                      :disabled="!row.enabled || settings.saving"
                      :open="expandedSelection === row.id"
                      @update:open="setSelectionOpen(row.id, $event)"
                      @change="settings.change($event)"
                    />
                    <Select
                      v-if="
                        row.id === 'network' ||
                        row.id === 'disk' ||
                        row.id === 'diskTemperature' ||
                        row.id === 'gpu' ||
                        row.id === 'gpuTemperature'
                      "
                      :disabled="row.id === 'gpuTemperature' ? !settings.draft || settings.saving : !row.enabled"
                      :open="expandedSelection === row.id"
                      :model-value="
                        row.id === 'network'
                          ? (settings.draft?.networkInterface ?? 'automatic')
                          : row.id === 'gpu' || row.id === 'gpuTemperature'
                            ? (settings.draft?.gpuAdapter ?? 'automatic')
                            : (settings.draft?.diskVolume ?? 'system')
                      "
                      @update:open="setSelectionOpen(row.id, $event)"
                      @update:model-value="
                        row.id === 'network'
                          ? settings.change({ networkInterface: $event === 'automatic' ? null : String($event) })
                          : row.id === 'gpu' || row.id === 'gpuTemperature'
                            ? settings.change({ gpuAdapter: $event === 'automatic' ? null : String($event) })
                            : settings.change({ diskVolume: $event === 'system' ? null : String($event) })
                      "
                    >
                      <SelectTrigger as-child>
                        <MdTooltip
                          :text="
                            row.id === 'network'
                              ? selectedInterfaceLabel
                              : row.id === 'gpu' || row.id === 'gpuTemperature'
                                ? selectedGpuLabel
                                : selectedVolumeLabel
                          "
                          ><button
                            type="button"
                            class="selection-toggle"
                            :aria-label="
                              row.id === 'network'
                                ? `${t('systemStatus.interface')}: ${selectedInterfaceLabel}`
                                : row.id === 'gpu' || row.id === 'gpuTemperature'
                                  ? `${t('systemStatus.gpu')}: ${selectedGpuLabel}`
                                  : `${t('systemStatus.volume')}: ${selectedVolumeLabel}`
                            "
                          >
                            <span class="truncate">{{
                              row.id === 'network'
                                ? selectedInterfaceLabel
                                : row.id === 'gpu' || row.id === 'gpuTemperature'
                                  ? selectedGpuLabel
                                  : selectedVolumeLabel
                            }}</span>
                            <MdIcon
                              :name="expandedSelection === row.id ? ICON_NAMES.chevronUp : ICON_NAMES.chevronDown"
                              :size="14"
                              class="shrink-0"
                            /></button
                        ></MdTooltip>
                      </SelectTrigger>
                      <SelectContent align="end" class="z-60 max-w-[min(20rem,calc(100vw-2rem))]">
                        <template v-if="row.id === 'network'">
                          <SelectItem value="automatic">{{ t('systemStatus.automatic') }}</SelectItem>
                          <SelectItem v-for="item in interfaces" :key="item.id" :value="item.id" class="break-all">
                            {{ item.name }}{{ item.connected ? '' : ` · ${t('systemStatus.disconnected')}` }}
                          </SelectItem>
                          <SelectItem v-if="missingInterface" :value="settings.draft!.networkInterface!">{{
                            t('systemStatus.savedDisconnected')
                          }}</SelectItem>
                        </template>
                        <template v-else-if="row.id === 'gpu' || row.id === 'gpuTemperature'">
                          <SelectItem value="automatic">{{ t('systemStatus.automatic') }}</SelectItem>
                          <SelectItem v-for="item in gpuAdapters" :key="item.id" :value="item.id" class="break-all">{{
                            item.name
                          }}</SelectItem>
                          <SelectItem v-if="missingGpu" :value="settings.draft!.gpuAdapter!">{{
                            t('systemStatus.savedDisconnected')
                          }}</SelectItem>
                        </template>
                        <template v-else>
                          <SelectItem value="system">{{ t('systemStatus.systemDisk') }}</SelectItem>
                          <SelectItem v-for="item in volumes" :key="item.id" :value="item.id" class="break-all">{{
                            item.name
                          }}</SelectItem>
                          <SelectItem v-if="missingVolume" :value="settings.draft!.diskVolume!">{{
                            t('systemStatus.savedDisconnected')
                          }}</SelectItem>
                        </template>
                      </SelectContent>
                    </Select>
                    <Button
                      v-if="row.id !== 'network'"
                      :id="`color-configure-${row.id}`"
                      variant="ghost"
                      size="sm"
                      class="color-toggle shrink-0 text-muted-foreground"
                      aria-haspopup="dialog"
                      :aria-controls="`color-editor-${row.id}`"
                      :aria-label="t('systemStatus.colorConfigure', { name: t(displayLabelKeys[row.id]) })"
                      :disabled="settings.saving"
                      @click="openColor(row.id)"
                    >
                      {{ t('systemStatus.colorAction') }}
                    </Button>
                  </div>
                </div>
              </template>
            </MdResidentMetricOrder>
          </div>
        </div>
        <p v-if="settings.error || (displayEnabled && catalogueError)" class="settings-feedback" role="status">
          <template v-if="settings.error"
            >{{ t('systemStatus.saveFailed') }}
            <button @click="settings.load()">{{ t('monitoring.refresh') }}</button></template
          ><template v-else-if="catalogueError"
            >{{ t('systemStatus.catalogueFailed') }}
            <button @click="loadCatalogue">{{ t('monitoring.refresh') }}</button></template
          >
        </p>
      </div>
      <MdDialogFooter>
        <Button :disabled="settings.saving" @click="settingsOpen = false">{{ t('systemStatus.done') }}</Button>
      </MdDialogFooter>
    </MdDialogContent>
  </Dialog>
  <MdStatusColorRuleEditor
    v-if="activeColor && settings.draft"
    :key="activeColor"
    :name="t(displayLabelKeys[activeColor])"
    :rule="settings.draft.colorRules.find(rule => rule.id === activeColor)!"
    :saving="settings.saving"
    :error="settings.error ? t('systemStatus.saveFailed') : undefined"
    @save="saveColor"
    @cancel="activeColor = null"
  />
  <MdWindowsTemperatureSetup
    v-if="!isMacOs && !isLinux"
    :open="temperatureSetupOpen"
    :select-display-item="selectTemperatureAfterSetup"
    @update:open="temperatureSetupOpen = $event"
  />
</template>
<style scoped>
@reference "@assets/main.css";
.display-options {
  display: grid;
  gap: 14px;
  min-width: 0;
}
.controls-heading {
  @apply text-muted-foreground;
  display: flex;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 4px 12px;
  margin-bottom: 8px;
  font-size: var(--font-content-secondary);
  font-weight: 400;
}
.controls-heading h3 {
  margin: 0;
  font: inherit;
  font-weight: 500;
}
.status-item {
  @apply border-b border-border/50 bg-transparent;
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 42px;
  padding: 4px 0;
}
.status-item:last-child {
  border-bottom-color: transparent;
}
.metric-row.moving {
  outline: 2px solid var(--ring);
  outline-offset: 1px;
}
.metric-sort-move {
  transition: transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.metric-row.pointer-moving {
  outline: 1px dashed var(--ring);
  outline-offset: -1px;
  /* Keep the destination visible immediately; only neighbouring cards make way. */
  transition: none;
}
.pointer-moving .metric-heading {
  visibility: hidden;
}
.status-item.drag-preview {
  @apply rounded-md bg-card border border-primary/40 shadow-lg;
  position: fixed;
  z-index: 100;
  margin: 0;
  pointer-events: none;
  will-change: transform;
  cursor: grabbing;
}
@media (prefers-reduced-motion: reduce) {
  .metric-sort-move {
    transition: none;
  }
}
.metric-heading {
  display: flex;
  align-items: center;
  width: 100%;
  min-width: 0;
  gap: 8px;
}
.status-item[data-metric='fanSpeed'] .color-toggle {
  grid-column: 4;
  grid-row: 1;
}
.status-item[data-metric='fanSpeed'] .metric-heading {
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr) auto auto;
  column-gap: 8px;
  row-gap: 0;
}
.metric-label-group {
  display: flex;
  align-items: center;
  flex: 1;
  flex-wrap: wrap;
  min-width: 0;
  gap: 4px 8px;
}
.metric-heading label,
.logo-label {
  @apply text-foreground;
  display: flex;
  align-items: center;
  flex: 1;
  gap: 10px;
  min-width: 0;
  min-height: 32px;
  font-size: var(--font-content-body);
  font-weight: 400;
  cursor: pointer;
  overflow-wrap: anywhere;
}
.status-item .metric-label-group label {
  flex: 0 1 auto;
}
.status-item[data-metric='gpuTemperature'] .metric-label-group {
  flex: none;
  flex-wrap: nowrap;
}
.status-item[data-metric='gpuTemperature'] label {
  flex: none;
  white-space: nowrap;
}
.status-item[data-metric='gpuTemperature'] .selection-toggle {
  flex: 0 1 auto;
  min-width: 0;
  margin-left: auto;
  justify-content: flex-end;
}
.logo-marker {
  display: grid;
  place-items: center;
  flex: none;
  width: 24px;
}
.drag-handle,
.selection-toggle {
  @apply text-foreground;
  @apply text-muted-foreground rounded;
  display: grid;
  place-items: center;
  flex: none;
  height: 32px;
  cursor: pointer;
}
.drag-handle {
  width: 24px;
  touch-action: none;
  user-select: none;
  cursor: grab;
}
.selection-toggle {
  display: flex;
  gap: 6px;
  min-width: 0;
  max-width: 55%;
  padding-inline: 8px;
  font-size: var(--font-content-body);
  font-weight: 400;
}
.drag-handle:active {
  cursor: grabbing;
}
.drag-handle:focus-visible,
.selection-toggle:focus-visible {
  outline: 2px solid var(--ring);
}
.drag-handle:hover,
.selection-toggle:hover {
  @apply bg-accent text-foreground;
}
.selection-toggle:disabled {
  opacity: 0.4;
  cursor: default;
}
@media (pointer: coarse) {
  .status-item[data-metric='fanSpeed'] .metric-heading {
    grid-template-columns: 44px minmax(0, 1fr) auto auto;
  }
  .status-item label,
  .drag-handle,
  .selection-toggle {
    min-height: 44px;
  }
  .drag-handle,
  .logo-marker {
    width: 44px;
  }
}
.settings-feedback {
  @apply text-destructive-text;
  padding-top: 8px;
  font-size: 11px;
  line-height: 1.5;
}
.settings-feedback button {
  text-decoration: underline;
  cursor: pointer;
}
</style>
