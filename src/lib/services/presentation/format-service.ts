import * as FormatUtils from '@/lib/utils/presentation/format';

const integerLocale = new Intl.NumberFormat().resolvedOptions().locale;
const dateContexts = new Map<string, { locale: string; timeZone: string }>();
const listLocales = new Map<string, string>();

/** Resolves WebView presentation defaults before calling deterministic formatters. */
export class FormatService {
  static integer(value: number): string {
    return FormatUtils.integer(value, integerLocale);
  }
  static dateTime(timestamp: number | null | undefined, locale?: string): string {
    const key = locale || 'system';
    let context = dateContexts.get(key);
    if (!context) {
      const resolved = new Intl.DateTimeFormat(locale || undefined).resolvedOptions();
      context = { locale: locale || resolved.locale, timeZone: resolved.timeZone };
      dateContexts.set(key, context);
    }
    return FormatUtils.dateTime(timestamp, context.locale, context.timeZone);
  }
  static list(values: readonly string[], locale?: string): string {
    const key = locale || 'system';
    let resolved = listLocales.get(key);
    if (!resolved) {
      resolved = locale || new Intl.ListFormat().resolvedOptions().locale;
      listLocales.set(key, resolved);
    }
    return FormatUtils.list(values, resolved);
  }
}
