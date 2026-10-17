<script setup lang="ts">
import type { TrendPoint } from '@/lib/models/system-resources/system-resources';
import MdResourceTrend from './md-resource-trend.vue';
defineProps<{ history: TrendPoint[]; observedAtMs: number; active: boolean; label: string; identity: string }>();
</script>
<template>
  <section class="temperature-details">
    <slot />
    <div class="temperature-chart">
      <slot name="placeholder">
        <MdResourceTrend
          v-if="history.length"
          :key="identity"
          metric="temperature"
          :history="history"
          :observed-at-ms="observedAtMs"
          :active="active"
          :label="label"
        />
      </slot>
    </div>
  </section>
</template>
<style scoped>
@reference "@assets/main.css";
.temperature-details {
  @apply rounded-xl border border-border bg-card;
  min-width: 0;
  padding: 10px 12px;
  flex: none;
  height: 90px;
  display: flex;
  flex-direction: column;
}
.temperature-chart {
  display: grid;
  align-items: center;
  flex: 1;
  min-height: 0;
}
.temperature-chart :deep(.resource-trend) {
  height: 28px;
  margin-top: 4px;
}
</style>
