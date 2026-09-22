import type { ReactNode } from 'react'
import { useState } from 'react'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { ChevronDown, Pin } from 'lucide-react'

export interface FacetRow {
  key: string
  label: ReactNode
  count: number
  /** This row's share of the facet's own total, 0 to 100. */
  percent: number
  labelClassName?: string
}

interface FacetPanelProps {
  title: string
  rows: FacetRow[]
  pinned: boolean
  onTogglePin: () => void
  emptyLabel?: string
  /** Rendered above the rows, inside the collapsible body -- e.g. a severity bar. */
  header?: ReactNode
}

/**
 * One field's breakdown: a title, a pin toggle, and its buckets each read as
 * a count and a share of the field's own total (never of every hit -- see
 * `facetShares`).
 *
 * Pinning is view-only, kept in the parent's state rather than sent
 * anywhere: it says which of the (few, fixed) fields this screen already
 * knows about a reader wants first, not a preference the platform stores.
 */
export function FacetPanel({
  title,
  rows,
  pinned,
  onTogglePin,
  emptyLabel = 'Nothing yet.',
  header,
}: FacetPanelProps) {
  const [open, setOpen] = useState(true)

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className={cn(pinned && 'rounded-md border border-primary/25 bg-primary/[0.04] p-2')}
    >
      <div className='flex items-center gap-1'>
        <CollapsibleTrigger className='flex flex-1 items-center gap-1.5 text-xs font-medium text-muted-foreground hover:text-foreground'>
          <ChevronDown className={cn('h-3.5 w-3.5 shrink-0 transition-transform', open && 'rotate-180')} />
          {title}
        </CollapsibleTrigger>

        <Button
          variant='ghost'
          size='icon'
          className='h-5 w-5 shrink-0 text-muted-foreground hover:text-foreground'
          onClick={onTogglePin}
          aria-pressed={pinned}
          title={pinned ? `Unpin ${title}` : `Pin ${title} to the top`}
        >
          <Pin className={cn('h-3 w-3', pinned && 'fill-current text-primary')} />
        </Button>
      </div>

      <CollapsibleContent className='mt-2 flex flex-col gap-1.5'>
        {header}

        {rows.length === 0 ? (
          <p className='text-xs text-muted-foreground'>{emptyLabel}</p>
        ) : (
          rows.map((row) => (
            <div key={row.key} className='flex items-center justify-between gap-2 text-xs'>
              <span className={cn('truncate text-foreground', row.labelClassName)}>{row.label}</span>
              <span className='shrink-0 tabular-nums text-muted-foreground'>
                {row.count}
                <span className='text-muted-foreground/60'> · {Math.round(row.percent)}%</span>
              </span>
            </div>
          ))
        )}
      </CollapsibleContent>
    </Collapsible>
  )
}
