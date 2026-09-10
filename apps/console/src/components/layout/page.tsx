import { cn } from '@/lib/utils'

export function Page({ children, className }: { children: React.ReactNode; className?: string }) {
  return <div className={cn('mx-auto w-full max-w-6xl space-y-8 p-6', className)}>{children}</div>
}

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: React.ReactNode
  description?: React.ReactNode
  actions?: React.ReactNode
}) {
  return (
    <header className='flex items-start justify-between gap-4'>
      <div className='min-w-0 space-y-1'>
        <h1 className='truncate text-xl font-semibold tracking-tight'>{title}</h1>
        {description && <p className='text-sm text-muted-foreground'>{description}</p>}
      </div>
      {actions && <div className='flex shrink-0 items-center gap-2'>{actions}</div>}
    </header>
  )
}

export function Section({
  title,
  actions,
  children,
}: {
  title: string
  actions?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <section className='space-y-3'>
      <div className='flex items-center justify-between gap-4'>
        <h2 className='text-sm font-medium text-muted-foreground'>{title}</h2>
        {actions}
      </div>
      {children}
    </section>
  )
}

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string
  description?: string
  action?: React.ReactNode
}) {
  return (
    <div className='flex flex-col items-center gap-3 rounded-lg border border-dashed px-6 py-12 text-center'>
      <p className='text-sm font-medium'>{title}</p>
      {description && <p className='max-w-md text-sm text-muted-foreground'>{description}</p>}
      {action}
    </div>
  )
}

export function DataTable({ children }: { children: React.ReactNode }) {
  return <div className='overflow-x-auto rounded-lg border'>{children}</div>
}
