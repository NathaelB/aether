import { cn } from '@/lib/utils'

export function Meter({ percent, className }: { percent: number; className?: string }) {
  const value = Math.min(100, Math.max(0, percent))
  const tone = value >= 90 ? 'bg-destructive' : value >= 75 ? 'bg-amber-500' : 'bg-primary'

  return (
    <div
      className={cn('h-1.5 w-full overflow-hidden rounded-full bg-muted', className)}
      role='progressbar'
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div className={cn('h-full rounded-full transition-all', tone)} style={{ width: `${value}%` }} />
    </div>
  )
}
