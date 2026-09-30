import { Card } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import type { AvailabilityFigure, AvailabilityView } from '../../../availability'

function Partial({ figure }: { figure: AvailabilityFigure }) {
  if (!figure.partial) return null
  return <span className='ml-2 text-xs font-normal text-muted-foreground'>partial window</span>
}

export function AvailabilityCard({ view }: { view: AvailabilityView }) {
  if (view.kind === 'hidden') return null

  return (
    <Card className='p-4'>
      <p className='text-sm text-muted-foreground'>Availability</p>
      {view.kind === 'loading' && <Skeleton className='mt-2 h-8 w-40' />}
      {view.kind === 'no-data' && (
        <p className='mt-1 text-lg font-semibold text-muted-foreground'>No data yet</p>
      )}
      {view.kind === 'error' && (
        <p className='mt-1 text-sm text-muted-foreground'>Availability could not be loaded.</p>
      )}
      {view.kind === 'ready' && (
        <div className='mt-1 flex flex-wrap items-baseline gap-x-8 gap-y-2'>
          <div>
            <p className='text-2xl font-semibold'>
              {view.headline.value}
              <Partial figure={view.headline} />
            </p>
            <p className='text-xs text-muted-foreground'>{view.headline.label}</p>
          </div>
          {view.others.map((other) => (
            <div key={other.label}>
              <p className='text-lg font-semibold'>
                {other.value}
                <Partial figure={other} />
              </p>
              <p className='text-xs text-muted-foreground'>{other.label}</p>
            </div>
          ))}
        </div>
      )}
    </Card>
  )
}
