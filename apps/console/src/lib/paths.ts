/**
 * Paths built from ids the router cannot know at compile time.
 *
 * Typed as `string` on purpose: the router's `to` is a union of route
 * patterns, and these are concrete paths with an id already in them. Going
 * through one place keeps the shape of a link in one place too.
 */
export function platformPath(path = ''): string {
  return `/platform${path}`
}

export function organisationPathFor(organisationId: string, path = ''): string {
  return `/organisations/${organisationId}${path}`
}

/**
 * Whether one path sits at or under another.
 *
 * Matching on `startsWith` alone is wrong at a segment boundary: it makes
 * `/deployments` cover `/deployments-archive`, which is a different page
 * entirely. A prefix only counts when the next character ends the segment.
 */
export function isUnder(pathname: string, prefix: string, exact = false): boolean {
  const here = trim(pathname)
  const target = trim(prefix)

  if (exact) return here === target
  if (here === target) return true

  return here.startsWith(`${target}/`)
}

/**
 * Whether this path deliberately belongs to no organisation.
 *
 * The console sends somebody with no organisation in the URL to their own,
 * because landing nowhere usually means arriving at the root. These are the
 * places where having no organisation is the answer rather than a gap:
 * without this, reaching one of them bounces straight back and the link that
 * led there looks like a button that does nothing.
 */
export function belongsToNoOrganisation(pathname: string): boolean {
  return (
    isUnder(pathname, '/platform') ||
    isUnder(pathname, '/organisations/create', true) ||
    // Somebody arriving on an invitation link is on their way into an
    // organisation and is not in one yet. Sent to theirs first, the page that
    // puts them in never runs; sent to the create form, they are asked to
    // start an organisation instead of joining the one they were invited to.
    isUnder(pathname, '/invitations/accept', true)
  )
}

function trim(path: string): string {
  return path.length > 1 && path.endsWith('/') ? path.slice(0, -1) : path
}
