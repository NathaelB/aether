import type { Schemas } from '@/api/api.client'
import { cn } from '@/lib/utils'
import {
  actionMarkers,
  maxCount,
  toBars,
  xFraction,
  type ActionMarker,
  type HistogramBar,
} from '../../histogram'

const WIDTH = 960
const HEIGHT = 72
const PADDING = { left: 4, right: 4, top: 6, bottom: 18 }

interface Props {
  buckets: Schemas.LogSearchBucket[]
  actions: Schemas.Action[]
  from: number
  to: number
}

/**
 * Log volume over the search window, with the deployment's own actions on
 * the same axis -- so a spike and the upgrade that caused it sit at the same
 * x, rather than in two screens a reader has to line up by hand.
 */
export function LogsHistogram({ buckets, actions, from, to }: Props) {
  const bars = toBars(buckets)
  const markers = actionMarkers(actions, from, to)

  if (bars.length === 0 && markers.length === 0) return null

  const tallest = maxCount(bars)
  const plotWidth = WIDTH - PADDING.left - PADDING.right
  const plotHeight = HEIGHT - PADDING.top - PADDING.bottom
  const barWidth = bars.length > 0 ? plotWidth / bars.length : 0

  const x = (at: number) => PADDING.left + xFraction(at, from, to) * plotWidth
  const barHeight = (bar: HistogramBar) => (bar.count / tallest) * plotHeight
  const barY = (bar: HistogramBar) => PADDING.top + plotHeight - barHeight(bar)

  return (
    <div className='overflow-x-auto rounded-md border bg-muted/10 px-1 pt-2'>
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        className='h-20 w-full'
        role='img'
        aria-label='Log volume over the search window, with matching deployment actions'
        preserveAspectRatio='none'
      >
        {bars.map((bar) => (
          <rect
            key={bar.at}
            x={x(bar.at)}
            y={barY(bar)}
            width={Math.max(1, barWidth - 1)}
            height={Math.max(0, barHeight(bar))}
            className={cn(bar.count > 0 ? 'fill-primary/70' : 'fill-transparent')}
          >
            <title>
              {new Date(bar.at).toISOString().slice(0, 19).replace('T', ' ')}Z — {bar.count}{' '}
              {bar.count === 1 ? 'line' : 'lines'}
            </title>
          </rect>
        ))}

        <line
          x1={PADDING.left}
          x2={WIDTH - PADDING.right}
          y1={PADDING.top + plotHeight}
          y2={PADDING.top + plotHeight}
          className='stroke-border'
          strokeWidth={1}
        />

        {markers.map((marker) => (
          <ActionMark key={`${marker.at}-${marker.label}`} marker={marker} x={x(marker.at)} />
        ))}
      </svg>
    </div>
  )
}

function ActionMark({ marker, x }: { marker: ActionMarker; x: number }) {
  return (
    <g>
      <line
        x1={x}
        x2={x}
        y1={PADDING.top}
        y2={HEIGHT - PADDING.bottom}
        className='stroke-amber-500 dark:stroke-amber-400'
        strokeWidth={1.5}
        strokeDasharray='2 2'
      />
      <circle
        cx={x}
        cy={HEIGHT - PADDING.bottom}
        r={3}
        className='fill-amber-500 dark:fill-amber-400'
      />
      <text
        x={x}
        y={HEIGHT - 4}
        textAnchor='middle'
        className='fill-amber-700 text-[8px] uppercase dark:fill-amber-300'
      >
        {marker.label}
      </text>
      <title>
        {new Date(marker.at).toISOString().slice(0, 19).replace('T', ' ')}Z — {marker.label}
      </title>
    </g>
  )
}
