/**
 * What is wrong with a restore's name, or nothing.
 *
 * Said before the request rather than after the refusal, the same reasoning
 * `checkForm` in `schedule.ts` follows for the schedule form.
 */
export function checkRestoreName(name: string): string | null {
  if (name.trim().length === 0) return 'The recovery needs a name of its own.'

  return null
}
