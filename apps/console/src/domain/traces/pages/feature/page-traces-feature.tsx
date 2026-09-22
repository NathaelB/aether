import { useEffect, useMemo, useRef, useState } from 'react'
import { useParams } from '@tanstack/react-router'
import type { ApiRequestError } from '@/api/api.fetch'
import { useGetDeployment } from '@/api/deployment.api'
import { useSearchTraces } from '@/api/traces.api'
import { SectionPage } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { DEFAULT_TRACE_WINDOW_MINUTES, buildTraceSearchRequest } from '../../search'
import { PageTracesSearch } from '../ui/page-traces-search'

/** Free text is searched as the reader types, but not on every keystroke. */
const TEXT_DEBOUNCE_MS = 300

/**
 * Traces are search-only from day one -- unlike logs, there is no live-tail
 * equivalent here, so this feature is just the one view `page-logs-feature`
 * shows on its Search tab.
 */
export default function PageTracesFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const deployment = useGetDeployment(deploymentId ?? null)

  const [windowMinutes, setWindowMinutes] = useState(DEFAULT_TRACE_WINDOW_MINUTES)
  const [serviceName, setServiceName] = useState('')
  const [statusCode, setStatusCode] = useState('')
  const [text, setText] = useState('')
  const [debouncedText, setDebouncedText] = useState('')

  useEffect(() => {
    const timeout = setTimeout(() => setDebouncedText(text), TEXT_DEBOUNCE_MS)
    return () => clearTimeout(timeout)
  }, [text])

  const request = useMemo(
    () =>
      deploymentId
        ? buildTraceSearchRequest(
            { windowMinutes, serviceName, statusCode, text: debouncedText },
            deploymentId,
            new Date(),
          )
        : null,
    [deploymentId, windowMinutes, serviceName, statusCode, debouncedText],
  )

  const search = useSearchTraces(organisationId, request, true)
  const searchError = search.error as ApiRequestError | null
  const errorStatus = search.isError ? (searchError?.status ?? 0) : null

  // Client-measured, the same reasoning as `page-logs-feature`'s own timer:
  // the trace search endpoint does not report its own timing either.
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

  return (
    <SectionPage title='Traces' description="A 30-day stored search of this deployment's spans.">
      {deployment.isLoading || !deployment.data ? (
        <div className='space-y-3'>
          <Skeleton className='h-8 w-full' />
          <Skeleton className='h-72 w-full' />
        </div>
      ) : (
        <PageTracesSearch
          scopeLabel={deployment.data.data.name}
          organisationId={organisationId}
          windowMinutes={windowMinutes}
          onWindowChange={setWindowMinutes}
          serviceName={serviceName}
          onServiceNameChange={setServiceName}
          statusCode={statusCode}
          onStatusCodeChange={setStatusCode}
          text={text}
          onTextChange={setText}
          onRun={() => void search.refetch()}
          result={search.data}
          isLoading={search.isLoading}
          elapsedMs={elapsedMs}
          errorStatus={errorStatus}
          windowFrom={request ? Date.parse(request.from) : 0}
          windowTo={request ? Date.parse(request.to) : 0}
        />
      )}
    </SectionPage>
  )
}
