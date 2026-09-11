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
