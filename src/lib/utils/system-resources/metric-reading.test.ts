import { describe, expect, it } from 'vitest';
import type { MetricReading, MetricStatus } from '@/lib/models/system-resources/system-resources';
import { hasFreshValue } from './metric-reading';

const reading = (): MetricReading<number> => ({ status: 'ready', value: 0, sampledAtMs: 1000 });

describe('metric observation freshness', () => {
  it('accepts zero values and the inclusive expiration boundary', () => {
    expect(hasFreshValue(reading(), 1000, 10_000)).toBe(true);
    expect(hasFreshValue(reading(), 11_000, 10_000)).toBe(true);
    expect(hasFreshValue(reading(), 11_001, 10_000)).toBe(false);
  });

  it.each<MetricStatus>(['loading', 'stale', 'failed', 'unsupported', 'disconnected'])(
    'rejects %s observations independently of their timestamp',
    status => expect(hasFreshValue({ ...reading(), status }, 1000, 10_000)).toBe(false)
  );

  it('rejects missing values, invalid timestamps and backwards clocks', () => {
    expect(hasFreshValue({ ...reading(), value: null }, 1000, 10_000)).toBe(false);
    for (const sampledAtMs of [null, NaN, Infinity, -Infinity]) {
      expect(hasFreshValue({ ...reading(), sampledAtMs }, 1000, 10_000)).toBe(false);
    }
    for (const now of [999, NaN, Infinity, -Infinity]) {
      expect(hasFreshValue(reading(), now, 10_000)).toBe(false);
    }
  });
});
