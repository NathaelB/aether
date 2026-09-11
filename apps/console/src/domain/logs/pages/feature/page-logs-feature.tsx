import { useCallback, useEffect, useRef, useState } from 'react'
import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { selectAccessToken, useAuthStore } from '@/stores/auth'
import { WINDOWS, keepRecent, readLogStream, type LogLine } from '../../stream'
import { PageLogs } from '../ui/page-logs'

export default function PageLogsFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const accessToken = useAuthStore(selectAccessToken)

  const deployment = useGetDeployment(deploymentId ?? null)
  const [minutes, setMinutes] = useState<number>(WINDOWS[1].minutes)
  const [isStreaming, setStreaming] = useState(true)
  const [lines, setLines] = useState<LogLine[]>([])
  const [error, setError] = useState<string | null>(null)

  const held = useRef<LogLine[]>([])

  const onLines = useCallback((arriving: LogLine[]) => {
    held.current = keepRecent(held.current, arriving)
    setLines(held.current)
  }, [])

  useEffect(() => {
    if (!organisationId || !deploymentId || !accessToken || !isStreaming) return

    const controller = new AbortController()

    readLogStream({
      url: `${window.apiUrl}/organisations/${organisationId}/deployments/${deploymentId}/logs?since_minutes=${minutes}`,
      token: accessToken,
      signal: controller.signal,
      onLines,
    }).catch((cause: unknown) => {
      if (controller.signal.aborted) return
      setError(
        cause instanceof Error
          ? `The stream ended: ${cause.message}`
          : 'The stream ended unexpectedly.',
      )
    })

    // Leaving the page ends the session. Nothing is left behind to come back
    // to, which is the whole point of relaying rather than storing.
    return () => controller.abort()
  }, [organisationId, deploymentId, accessToken, minutes, isStreaming, onLines])

  return (
    <PageLogs
      deployment={deployment.data?.data}
      lines={lines}
      minutes={minutes}
      onMinutesChange={(value) => {
        held.current = []
        setLines([])
        setError(null)
        setMinutes(value)
      }}
      isStreaming={isStreaming}
      onToggle={() => {
        // Resuming opens a fresh session over the same window, so the lines
        // already on screen would arrive a second time. Starting empty is
        // both simpler and more honest than pretending it is one long read.
        if (!isStreaming) {
          held.current = []
          setLines([])
        }
        setError(null)
        setStreaming((streaming) => !streaming)
      }}
      error={error}
      isLoading={deployment.isLoading}
    />
  )
}
