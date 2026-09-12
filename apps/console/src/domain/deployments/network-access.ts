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

/** What a set of ranges amounts to, said in one line. */
export function describeAccess(ranges: string[], hostname: string): string {
  if (ranges.length === 0) {
    return `${hostname} is reachable from anywhere.`
  }

  return ranges.length === 1
    ? `Only one range reaches ${hostname}. Every other address is refused, including this browser.`
    : `Only these ${ranges.length} ranges reach ${hostname}. Every other address is refused, including this browser.`
}

/** Why a range cannot be added, on top of the reasons it is not a range. */
export type AdditionProblem = RangeProblem | { kind: 'already-there' }

/**
 * Whether a range can join the list, and why not when it cannot.
 *
 * Checked before the range is added rather than after: the platform keeps
 * the first of a repeated pair and drops the rest silently, so a form that
 * accepted the second would report a change that never happened.
 */
export function checkAddition(applied: string[], raw: string): AdditionProblem | null {
  const value = raw.trim()
  if (value.length === 0) return { kind: 'not-a-range' }

  const problem = checkRange(value)
  if (problem) return problem

  return applied.includes(value) ? { kind: 'already-there' } : null
}

export function describeAdditionProblem(problem: AdditionProblem, value: string): string {
  if (problem.kind === 'already-there') return 'That range is already allowed.'

  return describeProblem(problem, value)
}

/** The list with one more range in it. */
export function withRange(applied: string[], raw: string): string[] {
  return [...applied, raw.trim()]
}

/** The list with one range gone. Empty means open, as it does everywhere. */
export function withoutRange(applied: string[], range: string): string[] {
  return applied.filter((entry) => entry !== range)
}

/**
 * What removing this range will do, when it is worth saying.
 *
 * Only for the last one. Warning on every removal would train people to
 * ignore the warning, and removing one of five changes who gets in without
 * changing whether anyone is kept out.
 */
export function describeRemoval(applied: string[], hostname: string): string | null {
  if (applied.length !== 1) return null

  return `Removing the last range makes ${hostname} reachable from anywhere again.`
}



