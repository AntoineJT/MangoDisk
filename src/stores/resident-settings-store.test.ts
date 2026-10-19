import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useResidentSettingsStore } from './resident-settings-store';
import { ResidentService } from '@/lib/services/resident/resident-service';
import { preferencesFixture } from '@/tests/fixtures/resident';
import type { ResidentPreferences } from '@/lib/models/resident/resident';

vi.mock('@/lib/services/resident/resident-service', () => ({
  ResidentService: { preferences: vi.fn(), savePreferences: vi.fn() },
}));
vi.mock('@/lib/services/diagnostics/logger-service', () => ({ LoggerService: { warn: vi.fn(), info: vi.fn() } }));

const conflict = {
  code: 'operationFailed',
  details: { operation: 'resident_save_preferences', reason: 'preferencesConflict' },
  retryable: true,
};

describe('resident preference transactions', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    vi.mocked(ResidentService.preferences).mockResolvedValue(preferencesFixture());
    vi.mocked(ResidentService.savePreferences).mockImplementation(async value => ({
      ...value,
      revision: value.revision + 1,
    }));
  });

  it('coalesces fifty edits behind one in-flight write and uses its committed revision', async () => {
    const store = useResidentSettingsStore();
    await store.load();
    let complete!: (value: ResidentPreferences) => void;
    vi.mocked(ResidentService.savePreferences).mockReturnValueOnce(
      new Promise(resolve => {
        complete = resolve;
      })
    );
    const first = store.change({ showIcon: false });
    for (let index = 0; index < 50; index++) await store.change({ networkInterface: `device-${index}` });
    expect(ResidentService.savePreferences).toHaveBeenCalledTimes(1);
    expect(store.draft?.networkInterface).toBe('device-49');
    expect(store.preferences?.networkInterface).toBeNull();
    complete({ ...preferencesFixture(), showIcon: false, revision: 1 });
    await first;
    expect(ResidentService.savePreferences).toHaveBeenCalledTimes(2);
    expect(vi.mocked(ResidentService.savePreferences).mock.calls[1]?.[0]).toMatchObject({
      revision: 1,
      networkInterface: 'device-49',
    });
    expect(store.preferences).toMatchObject({ revision: 2, networkInterface: 'device-49', showIcon: false });
    expect(store.saving).toBe(false);
  });

  it('rolls preview back to committed state after a rejected native transaction', async () => {
    const store = useResidentSettingsStore();
    await store.load();
    vi.mocked(ResidentService.savePreferences).mockRejectedValueOnce(new Error('disk full'));
    await store.change({ showIcon: false, diskVolume: 'uncommitted' });
    expect(store.error).toBe(true);
    expect(store.draft).toEqual(preferencesFixture());
    expect(store.preferences).toEqual(preferencesFixture());
    await store.change({ showIcon: false });
    expect(store.error).toBe(false);
    expect(store.preferences?.showIcon).toBe(false);
  });

  it('retains known preferences if reconciliation also fails', async () => {
    const store = useResidentSettingsStore();
    await store.load();
    vi.mocked(ResidentService.savePreferences).mockRejectedValueOnce(new Error('native failure'));
    vi.mocked(ResidentService.preferences).mockRejectedValueOnce(new Error('IPC failure'));
    await store.change({ enabled: false });
    expect(store.draft?.enabled).toBe(true);
    expect(store.error).toBe(true);
  });

  it('recovers a stale window edit without overwriting another window GPU choice', async () => {
    const store = useResidentSettingsStore();
    const previous = { ...preferencesFixture(), revision: 41 };
    vi.mocked(ResidentService.preferences).mockResolvedValue(previous);
    await store.load();
    const latest = { ...previous, revision: 44, gpuAdapter: 'intel' };
    vi.mocked(ResidentService.preferences).mockResolvedValue(latest);
    vi.mocked(ResidentService.savePreferences).mockRejectedValueOnce(conflict);
    await store.change({ diskVolume: 'data' });
    expect(ResidentService.savePreferences).toHaveBeenCalledTimes(2);
    expect(ResidentService.savePreferences).toHaveBeenLastCalledWith({ ...latest, diskVolume: 'data' });
    expect(store.preferences).toMatchObject({ revision: 45, gpuAdapter: 'intel', diskVolume: 'data' });
    expect(store.draft).toEqual(store.preferences);
    expect(store.error).toBe(false);
  });

  it('keeps rapid local edits made while the conflicting write is in flight', async () => {
    const store = useResidentSettingsStore();
    await store.load();
    let reject!: (error: unknown) => void;
    vi.mocked(ResidentService.savePreferences).mockReturnValueOnce(new Promise((_, fail) => (reject = fail)));
    const saving = store.change({ diskVolume: 'data' });
    await store.change({ networkInterface: 'wifi' });
    vi.mocked(ResidentService.preferences).mockResolvedValue({
      ...preferencesFixture(),
      revision: 3,
      gpuAdapter: 'intel',
    });
    reject(conflict);
    await saving;
    expect(store.preferences).toMatchObject({
      revision: 4,
      diskVolume: 'data',
      networkInterface: 'wifi',
      gpuAdapter: 'intel',
    });
    expect(store.error).toBe(false);
  });

  it('does not retry when both windows changed the same field differently', async () => {
    const store = useResidentSettingsStore();
    await store.load();
    const latest = { ...preferencesFixture(), revision: 2, gpuAdapter: 'amd' };
    vi.mocked(ResidentService.preferences).mockResolvedValue(latest);
    vi.mocked(ResidentService.savePreferences).mockRejectedValueOnce(conflict);
    await store.change({ gpuAdapter: 'intel' });
    expect(ResidentService.savePreferences).toHaveBeenCalledTimes(1);
    expect(store.draft).toEqual(latest);
    expect(store.error).toBe(true);
  });

  it('bounds recovery to one retry even if another writer commits again', async () => {
    const store = useResidentSettingsStore();
    await store.load();
    vi.mocked(ResidentService.preferences)
      .mockResolvedValueOnce({ ...preferencesFixture(), revision: 2, gpuAdapter: 'intel' })
      .mockResolvedValueOnce({ ...preferencesFixture(), revision: 3, gpuAdapter: 'amd' });
    vi.mocked(ResidentService.savePreferences).mockRejectedValue(conflict);
    await store.change({ diskVolume: 'data' });
    expect(ResidentService.savePreferences).toHaveBeenCalledTimes(2);
    expect(store.draft).toMatchObject({ revision: 3, gpuAdapter: 'amd', diskVolume: null });
    expect(store.error).toBe(true);
  });

  it('does not retry storage failures even when another window advanced the revision', async () => {
    const store = useResidentSettingsStore();
    await store.load();
    vi.mocked(ResidentService.preferences).mockResolvedValue({ ...preferencesFixture(), revision: 2 });
    vi.mocked(ResidentService.savePreferences).mockRejectedValueOnce({
      ...conflict,
      details: { operation: 'resident_save_preferences' },
    });
    await store.change({ diskVolume: 'data' });
    expect(ResidentService.savePreferences).toHaveBeenCalledTimes(1);
    expect(store.error).toBe(true);
  });

  it.each(['resolve', 'reject'] as const)(
    'ignores a stale retry that completes with %s after a newer edit',
    async outcome => {
      const store = useResidentSettingsStore();
      await store.load();
      let resolve!: (value: ResidentPreferences) => void;
      let reject!: (error: Error) => void;
      vi.mocked(ResidentService.preferences).mockReturnValueOnce(
        new Promise((success, failure) => {
          resolve = success;
          reject = failure;
        })
      );
      const retry = store.load();
      await store.change({ showIcon: false });
      if (outcome === 'resolve') resolve(preferencesFixture());
      else reject(new Error('late read failure'));
      await retry;
      expect(store.preferences).toMatchObject({ revision: 1, showIcon: false });
      expect(store.draft).toEqual(store.preferences);
      expect(store.error).toBe(false);
      expect(store.loading).toBe(false);
    }
  );
});
