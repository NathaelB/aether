import { cn } from '@/lib/utils'

export function Page({ children, className }: { children: React.ReactNode; className?: string }) {
  return <div className={cn('mx-auto w-full max-w-6xl px-6 py-8', className)}>{children}</div>
}

export function PageTitle({
  icon,
  title,
  badges,
  actions,
}: {
  icon?: React.ReactNode
  title: React.ReactNode
  badges?: React.ReactNode
  actions?: React.ReactNode
}) {
  return (
    <div className='space-y-4 border-b pb-5'>
      <div className='flex items-start justify-between gap-4'>
        <div className='flex min-w-0 items-center gap-3'>
          {icon}
          <h1 className='truncate text-2xl font-bold tracking-tight'>{title}</h1>
        </div>
        {actions && <div className='flex shrink-0 items-center gap-2'>{actions}</div>}
      </div>
      {badges && <div className='flex flex-wrap items-center gap-2'>{badges}</div>}
    </div>
  )
}

export function Section({
  title,
  aside,
  children,
  className,
}: {
  title: string
  aside?: React.ReactNode
  children: React.ReactNode
  className?: string
}) {
  return (
    <section className={cn('space-y-3', className)}>
      <div className='flex items-center justify-between gap-4'>
        <h2 className='text-base font-semibold'>{title}</h2>
        {aside}
      </div>
      {children}
    </section>
  )
}

export function Card({ children, className }: { children: React.ReactNode; className?: string }) {
  return <div className={cn('rounded-lg border bg-card p-5', className)}>{children}</div>
}

export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon?: React.ReactNode
  title: string
  description?: string
  action?: React.ReactNode
}) {
  return (
    <div className='flex flex-col items-center gap-2 rounded-lg border bg-muted/30 px-6 py-14 text-center'>
      {icon && (
        <div className='mb-2 flex h-10 w-10 items-center justify-center rounded-lg border bg-background text-muted-foreground'>
          {icon}
        </div>
      )}
      <p className='font-medium'>{title}</p>
      {description && <p className='max-w-md text-sm text-muted-foreground'>{description}</p>}
      {action && <div className='mt-3'>{action}</div>}
    </div>
  )
}

export function InfoRow({
  icon,
  label,
  value,
}: {
  icon?: React.ReactNode
  label: string
  value: React.ReactNode
}) {
  return (
    <div className='flex items-center justify-between gap-4 text-sm'>
      <span className='flex items-center gap-2 text-muted-foreground'>
        {icon}
        {label}
      </span>
      <span className='text-right font-medium'>{value}</span>
    </div>
  )
}
