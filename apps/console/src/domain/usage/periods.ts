export type PeriodKey = '1h' | '24h' | '7d'

export interface Period {
  key: PeriodKey
  label: string
  minutes: number
  /** How many points the chart draws. More than a few hundred is noise. */
  slots: number
}

export const PERIODS: Period[] = [
  { key: '1h', label: 'Last hour', minutes: 60, slots: 60 },
  { key: '24h', label: 'Last 24 hours', minutes: 60 * 24, slots: 96 },
  { key: '7d', label: 'Last 7 days', minutes: 60 * 24 * 7, slots: 168 },
]

const DEFAULT: PeriodKey = '24h'
const STORAGE_KEY = 'aether.usage.period'

export function periodFor(key: string | null | undefined): Period {
  return PERIODS.find((period) => period.key === key) ?? periodFor(DEFAULT)!
}

/**
 * The period the viewer last chose.
 *
 * Read through a try/catch because a browser with site data blocked throws on
 * the access itself rather than returning nothing, and a dashboard that
 * cannot render in a private window is worse than one that forgets a choice.
 */
export function rememberedPeriod(): Period {
  try {
    return periodFor(globalThis.localStorage.getItem(STORAGE_KEY))
  } catch {
    return periodFor(DEFAULT)
  }
}

export function rememberPeriod(key: PeriodKey): void {
  try {
    globalThis.localStorage.setItem(STORAGE_KEY, key)
  } catch {
    // Forgetting the choice is a small loss. Failing the render is not.
  }
}
