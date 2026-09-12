import { useEffect, useRef } from 'react'
import type { Schemas } from '@/api/api.client'
import { Page, PageTitle, Section } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { StatusBadge } from '@/components/ui/status-badge'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Pause, Play } from 'lucide-react'
import { WINDOWS, type LogLine } from '../../stream'

interface Props {
  deployment?: Schemas.Deployment
  lines: LogLine[]
  minutes: number
  onMinutesChange: (minutes: number) => void
  isStreaming: boolean
  onToggle: () => void
  error: string | null
  isLoading: boolean
}

export function PageLogs({
  deployment,
  lines,
  minutes,
  onMinutesChange,
  isStreaming,
  onToggle,
  error,
  isLoading,
}: Props) {
  const bottom = useRef<HTMLDivElement>(null)

  // Following the tail is what a log view is for. Pausing stops the scroll as
  // well as the stream, so a line someone is reading does not run away.
  useEffect(() => {
    if (isStreaming) bottom.current?.scrollIntoView({ block: 'end' })
  }, [lines, isStreaming])

  if (isLoading || !deployment) {
    return (
      <Page>
        <Skeleton className='h-9 w-64' />
        <Skeleton className='mt-8 h-80 w-full' />
      </Page>
    )
  }

  return (
    <Page>
      <PageTitle
        title='Logs'
        badges={<span className='text-xs text-muted-foreground'>{deployment.name}</span>}
        actions={
          <>
            <Tabs
              value={String(minutes)}
              onValueChange={(value) => onMinutesChange(Number(value))}
            >
              <TabsList>
                {WINDOWS.map(({ minutes: value, label }) => (
                  <TabsTrigger key={value} value={String(value)}>
                    {label}
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
            <Button variant='outline' size='sm' onClick={onToggle}>
              {isStreaming ? <Pause className='h-4 w-4' /> : <Play className='h-4 w-4' />}
              {isStreaming ? 'Pause' : 'Resume'}
            </Button>
          </>
        }
      />

      <div className='mt-8 space-y-4'>
        <Section
          title='Live'
          aside={
            <StatusBadge tone={isStreaming ? 'progress' : 'neutral'}>
              {isStreaming ? 'Streaming' : 'Paused'}
            </StatusBadge>
          }
        >
          {error && (
            <p className='rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200'>
              {error}
            </p>
          )}

          <div className='h-[28rem] overflow-auto rounded-lg border bg-muted/20 p-3 font-mono text-xs'>
            {lines.length === 0 ? (
              <p className='text-muted-foreground'>
                Waiting for the data plane to send. Nothing arrives until it does, and nothing is
                kept once you leave.
              </p>
            ) : (
              lines.map((line, index) => (
                <div key={`${line.at}-${index}`} className='flex gap-3 whitespace-pre-wrap py-0.5'>
                  <span className='shrink-0 text-muted-foreground tabular-nums'>
                    {new Date(line.at).toLocaleTimeString()}
                  </span>
                  <span className='shrink-0 text-muted-foreground'>{line.source}</span>
                  <span className='break-all'>{line.message}</span>
                </div>
              ))
            )}
            <div ref={bottom} />
          </div>

          <p className='text-xs text-muted-foreground'>
            Lines are relayed from your data plane and never stored by the platform. Each read is
            recorded in your audit log.
          </p>
        </Section>
      </div>
    </Page>
  )
}
