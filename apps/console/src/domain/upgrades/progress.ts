import { compareVersions } from './version'

/** Where an upgrade that spans several versions has got to. */
export interface UpgradeProgress {
  from: string
  to: string
  step: number
  of: number
  startedAt: string
}

/**
 * An upgrade the platform is in the middle of applying, as the API reports it.
 *
 * `steps` is the whole planned path and `current` is what the cluster says it
 * is running, so the step being applied is derived rather than counted. A
 * counter and a cluster disagree the moment one of them restarts.
 */
export interface InFlightUpgrade {
  from: string
  target: string
  steps: string[]
  current: string
  started_at: string
}

export function progressOf(inFlight: InFlightUpgrade | null | undefined): UpgradeProgress | null {
  if (!inFlight || inFlight.steps.length === 0) return null

  const done = inFlight.steps.filter((step) => compareVersions(step, inFlight.current) <= 0).length
  const step = Math.min(done + 1, inFlight.steps.length)

  return {
    from: inFlight.from,
    to: inFlight.target,
    step,
    of: inFlight.steps.length,
    startedAt: inFlight.started_at,
  }
}
