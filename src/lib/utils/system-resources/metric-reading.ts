import type { MetricReading } from '@/lib/models/system-resources/system-resources';

/** Check observation freshness with an explicit clock; metric-specific validation stays with its owner. */
export function hasFreshValue<T>(
  reading: MetricReading<T>,
  now: number,
  freshnessMs: number
): reading is MetricReading<T> & { value: T; sampledAtMs: number } {
  const sampled = reading.sampledAtMs;
  return (
    reading.status === 'ready' &&
    reading.value !== null &&
    sampled !== null &&
    Number.isFinite(sampled) &&
    Number.isFinite(now) &&
    now >= sampled &&
    now - sampled <= freshnessMs
  );
}
