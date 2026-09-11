/**
 * Whether a navigation entry points at where we are.
 *
 * Matching on `startsWith` alone is wrong at a segment boundary: it makes
 * `/deployments` light up on `/deployments-archive`, which is a different
 * page entirely. A prefix only counts when the next character ends the
 * segment.
 */
export function isActive(pathname: string, to: string, exact = false): boolean {
  const here = trim(pathname)
  const target = trim(to)

  if (exact) return here === target
  if (here === target) return true

  return here.startsWith(`${target}/`)
}

function trim(path: string): string {
  return path.length > 1 && path.endsWith('/') ? path.slice(0, -1) : path
}
