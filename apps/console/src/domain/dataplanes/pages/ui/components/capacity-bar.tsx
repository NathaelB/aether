import { cn } from '@/lib/utils'
import type { DimensionUsage } from '../../../capacity'

interface Props {
  label: string
  usage: DimensionUsage
  /** Renders the raw numbers, e.g. `2 / 4 vCPU`. */
  format: (value: number) => string
}

/**
 * Colour carries a placement meaning, not a mood.
 *
 * Amber starts where a data plane can no longer take another default-sized
 * deployment; red is where it is effectively full. Anything below is simply
 * room, and shading it would suggest a problem the operator does not have.
 *
 * The healthy bar is `bg-primary`, not a colour of its own. It is the one
 * value here that carries no warning, so it should be the product's colour --
 * and going through the token means it follows the theme instead of drifting
 * from it the first time the theme moves.
 */
function tone(percent: number): string {
  if (percent >= 90) return 'bg-destructive'
  if (percent >= 75) return 'bg-amber-500 dark:bg-amber-400'
  return 'bg-primary'
}

export function CapacityBar({ label, usage, format }: Props) {
  return (
    <div className='space-y-1.5'>
      <div className='flex items-baseline justify-between gap-2 text-sm'>
        <span className='text-muted-foreground'>{label}</span>
        <span className='font-mono text-xs'>
          <span className='font-medium text-foreground'>{format(usage.used)}</span>
          <span className='text-muted-foreground'> / {format(usage.total)}</span>
          <span className='text-muted-foreground'> · {usage.percent}%</span>
        </span>
      </div>
      <div
        className='h-1.5 w-full overflow-hidden rounded-full bg-muted'
        role='progressbar'
        aria-label={label}
        aria-valuenow={usage.percent}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div
          className={cn('h-full rounded-full transition-all', tone(usage.percent))}
          style={{ width: `${usage.percent}%` }}
        />
      </div>
    </div>
  )
}
