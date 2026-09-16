import { useCallback, useEffect, useRef, useState } from 'react'
import { selectAccessToken, useAuthStore } from '@/stores/auth'
import {
  StreamRefused,
  backoffMs,
  dropOverlap,
  keepRecent,
  readLogStream,
  whyItWasRefused,
  whyNotReopened,
  type LogLine,
} from '../stream'

/**
 * What the screen is allowed to say about the connection.
 *
 * Taken from the connection rather than from what the reader asked for. The
 * two used to be the same variable, which is how a stream that had ended left
 * a badge reading "Streaming" over a screen nothing would ever reach again.
 */
export type Connection =
  | { state: 'connecting' }
  | { state: 'live' }
  | { state: 'reconnecting' }
  | { state: 'paused' }
  /** Over, and not worth reopening. */
  | { state: 'stopped'; why: string }

interface Options {
  organisationId: string | null
  deploymentId: string | null
  minutes: number
  /** False while the reader has it paused. */
  running: boolean
}

/**
 * Follows a deployment's logs, reopening the session as often as it takes.
 *
 * A session is short-lived by design: the data plane stops following at its
 * own ceiling, and the control plane gives up on one that goes silent. Both
 * are ordinary, and neither is something a reader should have to notice --
 * so this reopens, and only says something when reopening would not help.
 */
export function useLogStream({ organisationId, deploymentId, minutes, running }: Options) {
  const accessToken = useAuthStore(selectAccessToken)

  const [lines, setLines] = useState<LogLine[]>([])
  const [following, setFollowing] = useState<Connection>({ state: 'connecting' })

  const held = useRef<LogLine[]>([])

  const forget = useCallback(() => {
    held.current = []
    setLines([])
  }, [])

  useEffect(() => {
    if (!organisationId || !deploymentId || !accessToken || !running) return

    const controller = new AbortController()
    let stopped = false
    let attempt = 0

    const onLines = (arriving: LogLine[]) => {
      // Reopening asks for the same window, so the first lines back are ones
      // already on screen.
      const fresh = dropOverlap(held.current, arriving)
      if (fresh.length === 0) return

      held.current = keepRecent(held.current, fresh)
      setLines(held.current)
    }

    const follow = async () => {
      while (!stopped) {
        setFollowing(attempt === 0 ? { state: 'connecting' } : { state: 'reconnecting' })

        try {
          const outcome = await readLogStream({
            url: `${window.apiUrl}/organisations/${organisationId}/deployments/${deploymentId}/logs?since_minutes=${minutes}`,
            token: accessToken,
            signal: controller.signal,
            onLines,
            onOpen: () => {
              // The read was accepted, so whatever went wrong before is over.
              attempt = 0
              setFollowing({ state: 'live' })
            },
          })

          if (stopped) return

          const why = whyNotReopened(outcome)
          if (why !== null) {
            setFollowing({ state: 'stopped', why })
            return
          }
        } catch (cause) {
          if (stopped || controller.signal.aborted) return

          // A refusal will be refused again. Reopening would spin against a
          // door closed on purpose, and hide the reason it is closed.
          if (cause instanceof StreamRefused) {
            setFollowing({ state: 'stopped', why: whyItWasRefused(cause.status) })
            return
          }
        }

        attempt += 1
        const wait = backoffMs(attempt)
        if (wait > 0) await new Promise((resume) => setTimeout(resume, wait))
      }
    }

    void follow()

    // Leaving the page ends the session. Nothing is left behind to come back
    // to, which is the whole point of relaying rather than storing.
    return () => {
      stopped = true
      controller.abort()
    }
  }, [organisationId, deploymentId, accessToken, minutes, running])

  // Paused is what the reader asked for rather than something the connection
  // is doing, so it is read off the request instead of being a state the
  // machine above has to be kept in step with.
  const connection: Connection = running ? following : { state: 'paused' }

  return { lines, connection, forget }
}
