<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { ResourceReadings } from '@/lib/models/system-resources';
import { ByteSizeService } from '@/lib/services/byte-size-service';
import MdTooltip from '@/components/custom/md-tooltip.vue';
import MdResourceOverview from './md-resource-overview.vue';
import MdResourceFacts from './md-resource-facts.vue';

const props = defineProps<{ reading: ResourceReadings; active: boolean }>();
const { t, n } = useI18n({ useScope: 'global' });
const network = computed(() => (props.reading.network.status === 'ready' ? props.reading.network.value : null));
function speed(bits: number | null | undefined): string {
  if (bits === null || bits === undefined || !Number.isFinite(bits) || bits <= 0) return '—';
  const scale = bits >= 1e9 ? 1e9 : bits >= 1e6 ? 1e6 : 1e3;
  return `${n(Number((bits / scale).toFixed(1)))} ${scale === 1e9 ? 'Gbps' : scale === 1e6 ? 'Mbps' : 'Kbps'}`;
}
const connection = computed(() => {
  const value = network.value;
  if (!value) return '—';
  if (value.interface.kind === 'wifi' && value.receiveLinkBps === null) return speed(value.transmitLinkBps);
  return value.receiveLinkBps === value.transmitLinkBps
    ? speed(value.receiveLinkBps)
    : `↓ ${speed(value.receiveLinkBps)} · ↑ ${speed(value.transmitLinkBps)}`;
});
const facts = computed(() => [
  {
    key: 'received',
    label: t('networkDetails.received'),
    value: network.value ? ByteSizeService.bytes(network.value.receivedBytesSinceStart) : '—',
    unit: '',
  },
  {
    key: 'transmitted',
    label: t('networkDetails.transmitted'),
    value: network.value ? ByteSizeService.bytes(network.value.transmittedBytesSinceStart) : '—',
    unit: '',
  },
]);
const interfaceFacts = computed(() => {
  const value = network.value?.interface;
  const rows = [
    { key: 'interface', label: t('systemStatus.interface'), value: value?.name ?? '—', unit: '' },
    {
      key: 'status',
      label: t('networkDetails.status'),
      value: value ? t(value.connected ? 'networkDetails.connected' : 'systemStatus.disconnected') : '—',
      unit: '',
    },
    {
      key: 'mac',
      label: t('networkDetails.hardwareAddress'),
      value: value?.details?.hardwareAddress ?? '—',
      unit: '',
      copy: true,
    },
    {
      key: 'ipv4',
      label: 'IPv4',
      value: value?.details?.addresses.filter(address => !address.includes(':')).join(', ') || '—',
      unit: '',
      copy: true,
    },
    {
      key: 'ipv6',
      label: 'IPv6',
      value: value?.details?.addresses.filter(address => address.includes(':')).join(', ') || '—',
      unit: '',
      copy: true,
    },
  ];
  const wifi = value?.details?.wifi;
  if (wifi) {
    rows.push({ key: 'standard', label: t('networkDetails.standard'), value: wifi.standard ?? '—', unit: '' });
    const channel = [
      wifi.bandGhz === null ? '' : `${n(wifi.bandGhz)} GHz`,
      wifi.channelWidthMhz === null ? '' : `${wifi.channelWidthMhz} MHz`,
    ]
      .filter(Boolean)
      .join(', ');
    rows.push({
      key: 'channel',
      label: t('networkDetails.channel'),
      value: wifi.channel === null ? '—' : `${wifi.channel}${channel ? ` (${channel})` : ''}`,
      unit: '',
    });
  }
  return rows;
});
</script>

<template>
  <MdResourceOverview metric="network" :reading="reading" :active="active" class="detail-summary network-summary" />
  <section class="network-details" :aria-label="t('networkDetails.title')">
    <header>
      <MdTooltip :active="active" :text="t('networkDetails.sessionHint')" multiline>
        <h2 tabindex="0">{{ t('networkDetails.title') }}</h2>
      </MdTooltip>
    </header>
    <MdResourceFacts :active="active" :facts="facts" />
  </section>
  <section class="network-details" :aria-label="t('systemStatus.interface')">
    <header>
      <h2>{{ t('systemStatus.interface') }}</h2>
      <MdTooltip
        :active="active"
        multiline
        :text="
          t(
            network?.interface.kind === 'wifi' && network.receiveLinkBps === null
              ? 'networkDetails.wifiLinkHint'
              : 'networkDetails.linkHint'
          )
        "
        ><span tabindex="0">{{ connection }}</span></MdTooltip
      >
    </header>
    <MdResourceFacts :active="active" :facts="interfaceFacts" layout="rows" />
  </section>
</template>

<style scoped>
@reference "@assets/main.css";
.network-details {
  @apply rounded-xl border border-border bg-card;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
h2 {
  font-size: 12px;
  font-weight: 600;
}
header span {
  @apply text-muted-foreground;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.network-summary {
  --resource-summary-height: 104px;
  --resource-trend-height: 28px;
}
</style>
