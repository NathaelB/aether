import { axisTicks, formatCount, segments, type Point } from '../../series'

const WIDTH = 640
const HEIGHT = 160
const PADDING = { left: 44, right: 8, top: 10, bottom: 22 }

interface Props {
  points: Point[]
  /** How to write an instant on the horizontal axis for this period. */
  formatTime: (at: number) => string
}

export function UsageChart({ points, formatTime }: Props) {
  const runs = segments(points)

  if (runs.length === 0) {
    return (
      <div className='flex h-40 items-center justify-center rounded-md border bg-muted/20 text-sm text-muted-foreground'>
        Nothing was reported for this period
      </div>
    )
  }

  const ticks = axisTicks(points)
  const low = ticks[0]
  const high = ticks[ticks.length - 1]
  // A flat line sits in the middle rather than on the floor, which would read
  // as zero even when the value is not.
  const span = high === low ? 1 : high - low

  const plotWidth = WIDTH - PADDING.left - PADDING.right
  const plotHeight = HEIGHT - PADDING.top - PADDING.bottom

  const x = (index: number) =>
    PADDING.left + (points.length <= 1 ? plotWidth / 2 : (index / (points.length - 1)) * plotWidth)

  const y = (value: number) =>
    high === low
      ? PADDING.top + plotHeight / 2
      : PADDING.top + plotHeight - ((value - low) / span) * plotHeight

  const indexOf = new Map(points.map((point, index) => [point, index]))

  return (
    <div className='overflow-x-auto rounded-md border p-3'>
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        className='h-40 w-full min-w-[520px]'
        role='img'
        aria-label='Usage over the selected period'
      >
        {ticks.map((tick) => (
          <g key={tick}>
            <line
              x1={PADDING.left}
              x2={WIDTH - PADDING.right}
              y1={y(tick)}
              y2={y(tick)}
              className='stroke-border'
              strokeWidth={1}
            />
            <text
              x={PADDING.left - 6}
              y={y(tick) + 3}
              textAnchor='end'
              className='fill-muted-foreground text-[10px] tabular-nums'
            >
              {formatCount(tick)}
            </text>
          </g>
        ))}

        {runs.map((run) => {
          const path = run
            .map((point, position) => {
              const index = indexOf.get(point) ?? position
              return `${position === 0 ? 'M' : 'L'} ${x(index)} ${y(point.value as number)}`
            })
            .join(' ')

          const key = `${indexOf.get(run[0])}-${run.length}`

          return run.length === 1 ? (
            <circle
              key={key}
              cx={x(indexOf.get(run[0]) ?? 0)}
              cy={y(run[0].value as number)}
              r={2}
              className='fill-primary'
            />
          ) : (
            <path key={key} d={path} fill='none' strokeWidth={2} className='stroke-primary' />
          )
        })}

        <text
          x={PADDING.left}
          y={HEIGHT - 6}
          className='fill-muted-foreground text-[10px] tabular-nums'
        >
          {formatTime(points[0].at)}
        </text>
        <text
          x={WIDTH - PADDING.right}
          y={HEIGHT - 6}
          textAnchor='end'
          className='fill-muted-foreground text-[10px] tabular-nums'
        >
          {formatTime(points[points.length - 1].at)}
        </text>
      </svg>
    </div>
  )
}
