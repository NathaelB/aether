import { useState } from 'react'
import { useParams } from '@tanstack/react-router'
import { useGetDeployment } from '@/api/deployment.api'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { useLogStream } from '../../hooks/use-log-stream'
import { WINDOWS } from '../../stream'
import { PageLogs } from '../ui/page-logs'

export default function PageLogsFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()

  const deployment = useGetDeployment(deploymentId ?? null)
  const [minutes, setMinutes] = useState<number>(WINDOWS[1].minutes)
  const [running, setRunning] = useState(true)

  const { lines, connection, forget } = useLogStream({
    organisationId,
    deploymentId: deploymentId ?? null,
    minutes,
    running,
  })

  return (
    <PageLogs
      deployment={deployment.data?.data}
      lines={lines}
      minutes={minutes}
      onMinutesChange={(value) => {
        // A different window is a different read, not a continuation of this
        // one: what is on screen was chosen by the old one.
        forget()
        setMinutes(value)
      }}
      connection={connection}
      // Resuming opens a fresh session over the same window, so the lines
      // already on screen arrive again -- and are dropped as the overlap they
      // are, which is why this no longer has to start empty.
      onToggle={() => setRunning((wasRunning) => !wasRunning)}
      isLoading={deployment.isLoading}
    />
  )
}
