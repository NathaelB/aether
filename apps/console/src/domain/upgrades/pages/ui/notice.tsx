import { Info } from 'lucide-react'

/** A statement of fact about how upgrades work, not a warning to dismiss. */
export function Notice({ children }: { children: React.ReactNode }) {
  return (
    <div className='flex items-start gap-2 rounded-md border bg-muted/40 px-3 py-2 text-sm text-muted-foreground'>
      <Info className='mt-0.5 h-4 w-4 shrink-0' />
      <span>{children}</span>
    </div>
  )
}
