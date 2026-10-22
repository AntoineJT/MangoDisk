<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import MdSettingsGroup from '@/components/custom/md-settings-group.vue';
import MdSettingsRow from '@/components/custom/md-settings-row.vue';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import type { ResidentPreferences } from '@/lib/models/resident/resident';

defineProps<{ preferences: ResidentPreferences }>();
const emit = defineEmits<{ change: [patch: Partial<ResidentPreferences>] }>();
const { t } = useI18n();
</script>
<template>
  <MdSettingsGroup plain>
    <MdSettingsRow compact :title="t('floatingMonitor.displayMode')" description="" label-for="windows-display-mode">
      <Select
        :model-value="preferences.windowsDisplayMode"
        @update:model-value="
          emit('change', { windowsDisplayMode: $event as ResidentPreferences['windowsDisplayMode'] })
        "
      >
        <SelectTrigger id="windows-display-mode" class="w-auto min-w-32"
          ><SelectValue>{{ t(`floatingMonitor.${preferences.windowsDisplayMode}`) }}</SelectValue></SelectTrigger
        >
        <SelectContent>
          <SelectItem v-for="mode in ['taskbar', 'floating', 'both'] as const" :key="mode" :value="mode">{{
            t(`floatingMonitor.${mode}`)
          }}</SelectItem>
        </SelectContent>
      </Select>
    </MdSettingsRow>
  </MdSettingsGroup>
</template>
