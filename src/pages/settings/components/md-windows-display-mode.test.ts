// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { describe, expect, it } from 'vitest';
import MdSwitch from '@/components/custom/md-switch.vue';
import Mode from './md-windows-display-mode.vue';
import Appearance from './md-floating-appearance.vue';
import Behavior from './md-floating-behavior.vue';
import { Select } from '@/components/ui/select';
import { preferencesFixture } from '@/tests/fixtures/resident';
import en from '@/locales/en-US.json';

const locales = Object.fromEntries(
  Object.entries(
    import.meta.glob<typeof en>('/src/locales/*.json', {
      eager: true,
      import: 'default',
    })
  ).map(([path, messages]) => [path.split('/').at(-1)!.replace('.json', ''), messages])
);
describe('Windows display mode settings', () => {
  it.each(Object.entries(locales))(
    'localizes floating settings and preserves unrelated options in %s',
    async (locale, messages) => {
      const preferences = preferencesFixture();
      const wrapper = mount(Mode, {
        props: { preferences },
        global: { plugins: [createI18n({ legacy: false, locale, messages: locales })] },
      });
      expect(wrapper.text()).toContain(messages.floatingMonitor.displayMode);
      expect(wrapper.find('#floating-layout').exists()).toBe(false);
      wrapper.findComponent(Select).vm.$emit('update:modelValue', 'floating');
      expect(wrapper.emitted('change')?.at(-1)).toEqual([{ windowsDisplayMode: 'floating' }]);
      await wrapper.setProps({ preferences: { ...preferences, windowsDisplayMode: 'floating' } });
      await flushPromises();
      expect(wrapper.find('#floating-layout').exists()).toBe(false);
      const behavior = mount(Behavior, {
        props: { preferences },
        global: { plugins: [createI18n({ legacy: false, locale, messages: locales })] },
      });
      const appearance = mount(Appearance, {
        props: { preferences },
        global: { plugins: [createI18n({ legacy: false, locale, messages: locales })] },
      });
      expect(behavior.text()).toContain(messages.floatingMonitor.alwaysOnTop);
      expect(behavior.text()).toContain(messages.floatingMonitor.snapToEdges);
      expect(behavior.text()).toContain(messages.floatingMonitor.clickThroughHint);
      behavior
        .findAllComponents(MdSwitch)
        .find(component => component.attributes('id') === 'floating-click-through')!
        .vm.$emit('update:modelValue', true);
      expect(behavior.emitted('change')?.at(-1)).toEqual([
        { floating: { ...preferences.floating, clickThrough: true } },
      ]);
      behavior
        .findAllComponents(MdSwitch)
        .find(component => component.attributes('id') === 'floating-snap')!
        .vm.$emit('update:modelValue', false);
      expect(behavior.emitted('change')?.at(-1)).toEqual([
        { floating: { ...preferences.floating, snapToEdges: false } },
      ]);
      appearance
        .findAllComponents(Select)
        .find(component => component.find('#floating-layout').exists())!
        .vm.$emit('update:modelValue', 'vertical');
      expect(appearance.emitted('change')?.at(-1)).toEqual([
        {
          floating: { ...preferences.floating, layout: 'vertical' },
        },
      ]);
      appearance
        .findAllComponents(Select)
        .find(component => component.find('#floating-opacity').exists())!
        .vm.$emit('update:modelValue', '60');
      expect(appearance.emitted('change')?.at(-1)).toEqual([
        { floating: { ...preferences.floating, backgroundOpacity: 60 } },
      ]);
      for (const [id, field, value] of [
        ['floating-font-size', 'fontSize', 'extraLarge'],
        ['floating-background', 'background', 'black'],
      ] as const) {
        appearance
          .findAllComponents(Select)
          .find(component => component.find(`#${id}`).exists())!
          .vm.$emit('update:modelValue', value);
        expect(appearance.emitted('change')?.at(-1)).toEqual([
          { floating: { ...preferences.floating, [field]: value } },
        ]);
      }
      appearance
        .findAllComponents(MdSwitch)
        .find(component => component.attributes('id') === 'floating-bold')!
        .vm.$emit('update:modelValue', true);
      expect(appearance.emitted('change')?.at(-1)).toEqual([{ floating: { ...preferences.floating, bold: true } }]);
      appearance
        .findAllComponents(MdSwitch)
        .find(component => component.attributes('id') === 'floating-dividers')!
        .vm.$emit('update:modelValue', false);
      expect(appearance.emitted('change')?.at(-1)).toEqual([
        { floating: { ...preferences.floating, dividers: false } },
      ]);
      expect(appearance.text()).toContain(messages.floatingMonitor.dividers);
      expect(appearance.text()).not.toMatch(/floatingMonitor\./u);
      behavior.unmount();
      appearance.unmount();
      wrapper.unmount();
    }
  );
  it('shows a valid saved percentage between the menu presets', async () => {
    const preferences = preferencesFixture();
    preferences.windowsDisplayMode = 'floating';
    preferences.floating.backgroundOpacity = 75;
    const wrapper = mount(Appearance, {
      props: { preferences },
      global: { plugins: [createI18n({ legacy: false, locale: 'en-US', messages: locales })] },
    });
    await flushPromises();
    expect(wrapper.get('#floating-opacity').text()).toContain('75%');
    wrapper.unmount();
  });
});
