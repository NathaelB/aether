import { useEffect, useMemo, useRef, useState } from 'react'
import { useParams } from '@tanstack/react-router'
import type { ApiRequestError } from '@/api/api.fetch'
import { useGetDeployment, useGetDeploymentActions } from '@/api/deployment.api'
import { useGroupLogs, useSearchLogs } from '@/api/logs.api'
import { SectionPage } from '@/components/layout/page'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { useLogStream } from '../../hooks/use-log-stream'
import {
  DEFAULT_SEARCH_LEVEL,
  DEFAULT_SEARCH_WINDOW_MINUTES,
  buildSearchRequest,
  type SearchLevel,
} from '../../search'
import { WINDOWS } from '../../stream'
import { PageLogs } from '../ui/page-logs'
import { PageLogsSearch, type ResultView } from '../ui/page-logs-search'

type Mode = 'live' | 'search'

/** Free text is searched as the reader types, but not on every keystroke. */
const TEXT_DEBOUNCE_MS = 300

export default function PageLogsFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const [mode, setMode] = useState<Mode>('live')

  const deployment = useGetDeployment(deploymentId ?? null)
  const [minutes, setMinutes] = useState<number>(WINDOWS[1].minutes)
  const [running, setRunning] = useState(true)

  const { lines, connection, forget } = useLogStream({
    organisationId,
    deploymentId: deploymentId ?? null,
    minutes,
    // The live session is the one thing this tab does that costs the data
    // plane something. Reading a stored, indexed copy on the Search tab is
    // no reason to keep it open.
    running: running && mode === 'live',
  })

  const [windowMinutes, setWindowMinutes] = useState(DEFAULT_SEARCH_WINDOW_MINUTES)
  const [floor, setFloor] = useState<SearchLevel>(DEFAULT_SEARCH_LEVEL)
  const [text, setText] = useState('')
  const [debouncedText, setDebouncedText] = useState('')
  const [view, setView] = useState<ResultView>('lines')

  useEffect(() => {
    const timeout = setTimeout(() => setDebouncedText(text), TEXT_DEBOUNCE_MS)
    return () => clearTimeout(timeout)
  }, [text])

  // Frozen to the moment one of the controls last changed, not recomputed on
  // every render: an absolute window has to stay the same request until the
  // reader asks for a different one.
  const request = useMemo(
    () =>
      deploymentId
        ? buildSearchRequest({ windowMinutes, floor, text: debouncedText }, deploymentId, new Date())
        : null,
    [deploymentId, windowMinutes, floor, debouncedText],
  )

  // Fetched whenever the Search tab is open, not only on the Lines view: the
  // histogram sits above both views and reads `search.data.buckets`, which
  // the Signatures view has no other way to reach.
  const search = useSearchLogs(organisationId, request, mode === 'search')
  const searchError = search.error as ApiRequestError | null
  const errorStatus = search.isError ? (searchError?.status ?? 0) : null

  // Client-measured, since the search endpoint does not report its own
  // timing: the round trip as this browser saw it, started the moment a
  // request went out and closed the moment react-query stopped fetching it.
  const requestStartedAt = useRef<number | null>(null)
  const [elapsedMs, setElapsedMs] = useState<number | null>(null)

  useEffect(() => {
    if (search.isFetching) {
      requestStartedAt.current = performance.now()
      return
    }

    if (requestStartedAt.current !== null) {
      setElapsedMs(performance.now() - requestStartedAt.current)
      requestStartedAt.current = null
    }
  }, [search.isFetching])

  const group = useGroupLogs(organisationId, request, mode === 'search' && view === 'signatures')
  const groupError = group.error as ApiRequestError | null
  const groupErrorStatus = group.isError ? (groupError?.status ?? 0) : null

  // The audit trail already records every deployment lifecycle action
  // (upgrade, restore, cutover, drill); read back here rather than through a
  // new endpoint, and placed on the same axis as the histogram (#298).
  const actions = useGetDeploymentActions(mode === 'search' ? (deploymentId ?? null) : null)

  const modeToggle = (
    <Tabs value={mode} onValueChange={(value) => setMode(value as Mode)}>
      <TabsList>
        <TabsTrigger value='live'>Live</TabsTrigger>
        <TabsTrigger value='search'>Search</TabsTrigger>
      </TabsList>
    </Tabs>
  )

  return (
    <SectionPage
      title='Logs'
      description="Live tail of this deployment's pods, or a 30-day stored search."
      actions={modeToggle}
    >
      {deployment.isLoading || !deployment.data ? (
        <PageLogs
          deployment={undefined}
          lines={lines}
          minutes={minutes}
          onMinutesChange={setMinutes}
          connection={connection}
          onToggle={() => setRunning((wasRunning) => !wasRunning)}
          isLoading
        />
      ) : mode === 'live' ? (
        <PageLogs
          deployment={deployment.data.data}
          lines={lines}
          minutes={minutes}
          onMinutesChange={(value) => {
            // A different window is a different read, not a continuation of
            // this one: what is on screen was chosen by the old one.
            forget()
            setMinutes(value)
          }}
          connection={connection}
          // Resuming opens a fresh session over the same window, so the lines
          // already on screen arrive again -- and are dropped as the overlap
          // they are, which is why this no longer has to start empty.
          onToggle={() => setRunning((wasRunning) => !wasRunning)}
          isLoading={false}
        />
      ) : (
        <PageLogsSearch
          scopeLabel={deployment.data.data.name}
          windowMinutes={windowMinutes}
          onWindowChange={setWindowMinutes}
          floor={floor}
          onFloorChange={setFloor}
          text={text}
          onTextChange={setText}
          onRun={() => void search.refetch()}
          view={view}
          onViewChange={setView}
          result={search.data}
          isLoading={search.isLoading}
          elapsedMs={elapsedMs}
          errorStatus={errorStatus}
          groupResult={group.data}
          isGrouping={group.isLoading}
          groupErrorStatus={groupErrorStatus}
          actions={actions.data?.data ?? []}
          windowFrom={request ? Date.parse(request.from) : 0}
          windowTo={request ? Date.parse(request.to) : 0}
        />
      )}
    </SectionPage>
  )
}
