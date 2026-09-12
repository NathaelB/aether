import type { Schemas } from '@/api/api.client'

export type NetworkAccess = Schemas.NetworkAccess

/**
 * What is wrong with a range somebody typed, or nothing.
 *
 * The same three rules the platform enforces, checked here so a mistake is
 * caught while the person still has their hands on the field rather than
 * after a round trip that loses their place. The platform still checks: this
 * is the faster of two answers, not the only one.
 */
export type RangeProblem =
  | { kind: 'not-a-range' }
  | { kind: 'prefix-too-long'; width: number }
  | { kind: 'host-bits-set'; network: string }

const V4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/

export function checkRange(raw: string): RangeProblem | null {
  const value = raw.trim()
  const slash = value.lastIndexOf('/')
  if (slash <= 0 || slash === value.length - 1) return { kind: 'not-a-range' }

  const address = value.slice(0, slash)
  const prefixText = value.slice(slash + 1)
  if (!/^\d+$/.test(prefixText)) return { kind: 'not-a-range' }
  const prefix = Number(prefixText)

  const octets = v4Octets(address)
  if (octets) {
    if (prefix > 32) return { kind: 'prefix-too-long', width: 32 }
    const network = maskV4(octets, prefix)
    if (network !== address) return { kind: 'host-bits-set', network: `${network}/${prefix}` }
    return null
  }

  // v6 is checked for shape and width only. Masking it here would mean
  // reimplementing address expansion in a form field, and getting that subtly
  // wrong is worse than letting the platform answer: it would reject ranges
  // that are perfectly good.
  if (isV6(address)) {
    if (prefix > 128) return { kind: 'prefix-too-long', width: 128 }
    return null
  }

  return { kind: 'not-a-range' }
}

function v4Octets(address: string): number[] | null {
  const match = V4.exec(address)
  if (!match) return null

  const octets = match.slice(1, 5).map(Number)
  if (octets.some((octet) => octet > 255)) return null
  // '01.2.3.4' parses as 1.2.3.4 and reads as something else. Refused rather
  // than normalised, so what is stored is what was written.
  if (match.slice(1, 5).some((part) => part.length > 1 && part.startsWith('0'))) return null

  return octets
}

function maskV4(octets: number[], prefix: number): string {
  const bits = octets.reduce((total, octet) => total * 256 + octet, 0)
  const kept = prefix === 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0
  const masked = (bits & kept) >>> 0

  return [24, 16, 8, 0].map((shift) => (masked >>> shift) & 255).join('.')
}

function isV6(address: string): boolean {
  if (!address.includes(':')) return false
  if (!/^[0-9a-fA-F:]+$/.test(address)) return false
  // '::' may appear once. Two of them is not an address, and neither is a
  // lone colon at either end.
  return address.split('::').length <= 2
}

export function describeProblem(problem: RangeProblem, value: string): string {
  switch (problem.kind) {
    case 'not-a-range':
      return `"${value}" is not a range. Write it like 203.0.113.0/24.`
    case 'prefix-too-long':
      return `An address of this kind stops at /${problem.width}.`
    case 'host-bits-set':
      return `That is an address, not a network. Did you mean ${problem.network}?`
  }
}

/**
 * The rule a list of entries amounts to.
 *
 * Nothing at all is open, never "restricted to nobody". Somebody removing
 * their last entry means stop restricting this, and it is the only reading
 * that leaves their identity provider reachable.
 */
export function accessFrom(entries: string[]): NetworkAccess {
  const ranges = entries.map((entry) => entry.trim()).filter((entry) => entry.length > 0)

  if (ranges.length === 0) return { kind: 'open' }

  return { kind: 'restricted', allowed: ranges }
}

export function rangesOf(access: NetworkAccess | undefined): string[] {
  return access && access.kind === 'restricted' ? access.allowed : []
}

export function isOpen(access: NetworkAccess | undefined): boolean {
  return !access || access.kind === 'open'
}

/** What the entries in the form amount to, said in one line. */
export function describeAccess(entries: string[], hostname: string): string {
  const ranges = accessFrom(entries)

  if (ranges.kind === 'open') {
    return `${hostname} is reachable from anywhere.`
  }

  const count = ranges.allowed.length
  return count === 1
    ? `Only one range will reach ${hostname}. Every other address is refused, including this browser.`
    : `Only these ${count} ranges will reach ${hostname}. Every other address is refused, including this browser.`
}

/**
 * Whether the form says something different from what is applied.
 *
 * Order counts as a change even though the platform does not care, because a
 * reordered list is still an edit somebody made and a Save that does nothing
 * is worse than one that writes the same thing.
 */
export function hasChanges(entries: string[], applied: NetworkAccess | undefined): boolean {
  const next = accessFrom(entries)
  const current = rangesOf(applied)

  if (next.kind === 'open') return current.length > 0
  if (next.allowed.length !== current.length) return true

  return next.allowed.some((range, index) => range !== current[index])
}

/** Every problem in the form, by position, so the fields can be marked. */
export function problems(entries: string[]): Map<number, RangeProblem> {
  const found = new Map<number, RangeProblem>()

  entries.forEach((entry, index) => {
    if (entry.trim().length === 0) return
    const problem = checkRange(entry)
    if (problem) found.set(index, problem)
  })

  return found
}

/** A range written twice does nothing, and saying so beats silently dropping it. */
export function duplicates(entries: string[]): Set<number> {
  const seen = new Map<string, number>()
  const repeated = new Set<number>()

  entries.forEach((entry, index) => {
    const value = entry.trim()
    if (value.length === 0) return
    if (seen.has(value)) repeated.add(index)
    else seen.set(value, index)
  })

  return repeated
}
