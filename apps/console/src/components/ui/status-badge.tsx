import { cn } from '@/lib/utils'

export type Tone = 'success' | 'progress' | 'warning' | 'danger' | 'neutral' | 'accent'

const TONES: Record<Tone, { pill: string; dot: string }> = {
  success: {
    pill: 'border-green-200 bg-green-50 text-green-700 dark:border-green-900 dark:bg-green-950 dark:text-green-300',
    dot: 'bg-green-500',
  },
  progress: {
    pill: 'border-blue-200 bg-blue-50 text-blue-700 dark:border-blue-900 dark:bg-blue-950 dark:text-blue-300',
    dot: 'bg-blue-500 animate-pulse',
  },
  warning: {
    pill: 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-300',
    dot: 'bg-amber-500',
  },
  danger: {
    pill: 'border-red-200 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300',
    dot: 'bg-red-500',
  },
  neutral: { pill: 'border-border bg-muted text-muted-foreground', dot: 'bg-muted-foreground' },
  accent: { pill: 'border-primary/30 bg-primary/10 text-primary', dot: 'bg-primary' },
}

export function StatusBadge({
  tone,
  children,
  dot = true,
  icon,
}: {
  tone: Tone
  children: React.ReactNode
  dot?: boolean
  icon?: React.ReactNode
}) {
  const { pill, dot: dotClass } = TONES[tone]

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-xs font-medium',
        pill,
      )}
    >
      {icon}
      {children}
      {dot && <span className={cn('h-1.5 w-1.5 rounded-full', dotClass)} />}
    </span>
  )
}
