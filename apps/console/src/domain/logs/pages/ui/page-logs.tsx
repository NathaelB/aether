import { useEffect, useMemo, useRef, useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { Page, PageTitle } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { StatusBadge, type Tone } from '@/components/ui/status-badge'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { cn } from '@/lib/utils'
import { ArrowDown, ChevronRight, Copy, Download, Pause, Play, Search } from 'lucide-react'
import type { Connection } from '../../hooks/use-log-stream'
import { narrow, toRecord, type Level, type LogRecord } from '../../record'
import { WINDOWS, type LogLine } from '../../stream'
import { asText, atTheEnd, readStamp, toneFor, type Clock } from '../../view'

interface Props {
  deployment?: Schemas.Deployment
  lines: LogLine[]
  minutes: number
  onMinutesChange: (minutes: number) => void
  connection: Connection
  onToggle: () => void
  isLoading: boolean
}

/** What the badge says, and how loudly, for each state of the connection. */
const SAID: Record<Connection['state'], { label: string; tone: Tone }> = {
  connecting: { label: 'Connecting', tone: 'progress' },
  live: { label: 'Live', tone: 'progress' },
  reconnecting: { label: 'Reconnecting', tone: 'warning' },
  paused: { label: 'Paused', tone: 'neutral' },
  stopped: { label: 'Ended', tone: 'neutral' },
}

/**
 * The tones containers are told apart by.
 *
 * Six, matching `SOURCE_TONES`. Chosen to stay legible on the surface the
 * lines sit on in both themes.
 */
const TONE_CLASSES = [
  'text-sky-600 dark:text-sky-400',
  'text-emerald-600 dark:text-emerald-400',
  'text-violet-600 dark:text-violet-400',
  'text-amber-600 dark:text-amber-400',
  'text-rose-600 dark:text-rose-400',
  'text-teal-600 dark:text-teal-400',
]

/**
 * Severity, drawn as severity.
 *
 * Only the two that carry news are coloured. Trace and debug are the bulk of
 * any log, and colouring them would make the screen a rainbow in which
 * nothing stands out -- which is the same as colouring nothing.
 */
const LEVEL_CLASSES: Record<Level, string> = {
  trace: 'text-muted-foreground/60',
  debug: 'text-muted-foreground',
  info: 'text-foreground',
  warn: 'text-amber-600 dark:text-amber-400',
  error: 'text-red-600 dark:text-red-400',
}

/** The floors worth offering. */
const FLOORS: { value: Level; label: string }[] = [
  { value: 'trace', label: 'Everything' },
  { value: 'debug', label: 'Debug and above' },
  { value: 'info', label: 'Info and above' },
  { value: 'warn', label: 'Warnings and errors' },
  { value: 'error', label: 'Errors only' },
]

/**
 * How long a message is before it is worth folding.
 *
 * A stack trace or a JSON body wrapped in full pushes everything around it
 * off the screen; folded, it is one line with the rest a click away.
 */
const LONG = 200

function Line({
  record,
  clock,
  showSource,
}: {
  record: LogRecord
  clock: Clock
  /** False when the line above came from the same container. */
  showSource: boolean
}) {
  const [open, setOpen] = useState(false)
  const long = record.text.length > LONG

  return (
    <div className='group flex gap-3 px-3 py-[3px] hover:bg-foreground/[0.04]'>
      {long ? (
        <button
          type='button'
          onClick={() => setOpen((wasOpen) => !wasOpen)}
          className='mt-[2px] shrink-0 text-muted-foreground transition-transform hover:text-foreground'
          aria-label={open ? 'Fold this line' : 'Unfold this line'}
          aria-expanded={open}
        >
          <ChevronRight className={cn('h-3 w-3', open && 'rotate-90')} />
        </button>
      ) : (
        <span className='w-3 shrink-0' aria-hidden />
      )}

      {/* Held blank rather than removed when it repeats: the column stays put,
          and a run of lines from one container reads as one run. */}
      <span
        className={cn('w-28 shrink-0 truncate', TONE_CLASSES[toneFor(record.source)])}
        title={record.source}
      >
        {showSource ? record.source : ''}
      </span>

      <time className='shrink-0 tabular-nums text-muted-foreground' dateTime={record.at}>
        {readStamp(record.at, clock)}
      </time>

      <span
        className={cn(
          'w-11 shrink-0 text-right text-[10px] uppercase leading-4',
          record.level ? LEVEL_CLASSES[record.level] : 'text-transparent',
        )}
      >
        {record.level ?? ''}
      </span>

      {record.target && (
        <span
          className='hidden w-44 shrink-0 truncate text-muted-foreground/70 lg:inline'
          title={record.target}
        >
          {record.target}
        </span>
      )}

      <span
        className={cn(
          'min-w-0 flex-1',
          record.level ? LEVEL_CLASSES[record.level] : undefined,
          open ? 'whitespace-pre-wrap break-words' : 'truncate',
        )}
      >
        {record.text}
      </span>
    </div>
  )
}

export function PageLogs({
  deployment,
  lines,
  minutes,
  onMinutesChange,
  connection,
  onToggle,
  isLoading,
}: Props) {
  const scroller = useRef<HTMLDivElement>(null)
  const [query, setQuery] = useState('')
  const [floor, setFloor] = useState<Level>('trace')
  const [clock, setClock] = useState<Clock>('browser')
  // Whether the tail is still being followed. Set by where the reader is
  // rather than by a control: scrolling up to read a line is the only signal
  // anybody gives that they want it to stay put.
  const [pinned, setPinned] = useState(true)

  const following = connection.state !== 'paused'

  // Read once per batch rather than once per render: the parse runs over
  // every line held, and re-reading a thousand of them on each keystroke in
  // the filter is a thousand regexes per character.
  const records = useMemo(() => lines.map(toRecord), [lines])
  const shown = narrow(records, floor, query)

  useEffect(() => {
    if (!pinned || !following) return

    const element = scroller.current
    if (element) element.scrollTop = element.scrollHeight
  }, [shown.length, pinned, following])

  if (isLoading || !deployment) {
    return (
      <Page>
        <Skeleton className='h-9 w-64' />
        <Skeleton className='mt-8 h-80 w-full' />
      </Page>
    )
  }

  const toEnd = () => {
    const element = scroller.current
    if (element) element.scrollTop = element.scrollHeight
    setPinned(true)
  }

  const download = () => {
    const file = new Blob([asText(shown, clock)], { type: 'text/plain' })
    const href = URL.createObjectURL(file)
    const link = document.createElement('a')

    link.href = href
    link.download = `${deployment.name}-logs.txt`
    link.click()

    URL.revokeObjectURL(href)
  }

  const narrowed = shown.length !== records.length

  return (
    <Page className='max-w-none'>
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
              {following ? <Pause className='h-4 w-4' /> : <Play className='h-4 w-4' />}
              {following ? 'Pause' : 'Resume'}
            </Button>
          </>
        }
      />

      <div className='mt-6 flex flex-col gap-3'>
        <div className='flex flex-wrap items-center gap-2'>
          <div className='relative min-w-56 flex-1'>
            <Search className='pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground' />
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder='Filter the lines on screen'
              className='h-8 pl-8 text-sm'
              aria-label='Filter the lines on screen'
            />
          </div>

          <Select value={floor} onValueChange={(value) => setFloor(value as Level)}>
            <SelectTrigger className='h-8 w-48 text-sm' aria-label='Least severe level to show'>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {FLOORS.map(({ value, label }) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <Tabs value={clock} onValueChange={(value) => setClock(value as Clock)}>
            <TabsList>
              <TabsTrigger value='browser'>Your time</TabsTrigger>
              <TabsTrigger value='utc'>UTC</TabsTrigger>
            </TabsList>
          </Tabs>

          <Button
            variant='outline'
            size='sm'
            onClick={() => void navigator.clipboard?.writeText(asText(shown, clock))}
            disabled={shown.length === 0}
          >
            <Copy className='h-3.5 w-3.5' />
            Copy
          </Button>

          <Button variant='outline' size='sm' onClick={download} disabled={shown.length === 0}>
            <Download className='h-3.5 w-3.5' />
            Download
          </Button>

          <StatusBadge tone={SAID[connection.state].tone}>
            {SAID[connection.state].label}
          </StatusBadge>
        </div>

        {connection.state === 'stopped' && (
          <p className='rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200'>
            {connection.why}
          </p>
        )}

        <div className='relative'>
          <div
            ref={scroller}
            onScroll={(event) => setPinned(atTheEnd(event.currentTarget))}
            // Tall enough to be the page rather than a panel on it. The
            // subtraction is the chrome above it: the header, this page's own
            // padding, the title and the toolbar.
            className='h-[calc(100svh-21rem)] min-h-80 overflow-auto rounded-lg border bg-muted/20 py-2 font-mono text-xs'
          >
            {shown.length === 0 ? (
              <p className='px-3 py-2 text-muted-foreground'>
                {records.length === 0
                  ? 'Waiting for the data plane to send. Nothing arrives until it does, and nothing is kept once you leave.'
                  : 'Nothing on screen matches. Widen the filter, or lower the level.'}
              </p>
            ) : (
              shown.map((record, index) => (
                <Line
                  key={`${record.at}-${index}`}
                  record={record}
                  clock={clock}
                  showSource={record.source !== shown[index - 1]?.source}
                />
              ))
            )}
          </div>

          {!pinned && (
            <Button size='sm' onClick={toEnd} className='absolute bottom-3 right-4 shadow-md'>
              <ArrowDown className='h-3.5 w-3.5' />
              Latest
            </Button>
          )}
        </div>

        <p className='flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground'>
          <span>
            Lines are relayed from your data plane and never stored by the platform. Each read is
            recorded in your audit log.
          </span>
          <span className='tabular-nums'>
            {narrowed ? `${shown.length} of ${records.length} lines` : `${records.length} lines held`}
          </span>
        </p>
      </div>
    </Page>
  )
}
