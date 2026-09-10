import { cn } from '@/lib/utils'
import { Check } from 'lucide-react'

interface Props {
  selected: boolean
  onSelect: () => void
  label: string
  description?: string
  footer?: React.ReactNode
}

export function OptionCard({ selected, onSelect, label, description, footer }: Props) {
  return (
    <button
      type='button'
      onClick={onSelect}
      aria-pressed={selected}
      className={cn(
        'relative flex flex-col gap-1 rounded-lg border p-4 text-left transition-colors',
        'hover:border-primary/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        selected ? 'border-primary bg-primary/5' : 'border-border',
      )}
    >
      {selected && <Check className='absolute right-3 top-3 h-4 w-4 text-primary' />}
      <span className='pr-6 font-medium'>{label}</span>
      {description && <span className='text-sm text-muted-foreground'>{description}</span>}
      {footer && <span className='mt-2'>{footer}</span>}
    </button>
  )
}
