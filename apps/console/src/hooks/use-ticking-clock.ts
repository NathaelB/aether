import { useEffect, useState } from 'react'

/**
 * A clock that moves, for screens showing how long something has been running.
 *
 * Without it "started 2 minutes ago" stays at two minutes until something
 * else happens to re-render, and a screen that does not move reads as a screen
 * that has stopped working.
 */
export function useTickingClock(everyMs: number): number {
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), everyMs)
    return () => clearInterval(tick)
  }, [everyMs])

  return now
}
