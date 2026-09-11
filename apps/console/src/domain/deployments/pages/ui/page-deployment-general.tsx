import type { Schemas } from '@/api/api.client'
import { SettingsPage } from '@/components/layout/page'
import { Skeleton } from '@/components/ui/skeleton'
import { format } from 'date-fns'
import { KIND_LABELS } from '../../types/deployment'
import { environmentOf } from '../../types/deployment'

interface Props {
  deployment?: Schemas.Deployment
  isLoading: boolean
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className='grid gap-1 border-b py-3 last:border-b-0 sm:grid-cols-3 sm:items-center sm:gap-4'>
      <dt className='text-sm text-muted-foreground'>{label}</dt>
      <dd className='text-sm sm:col-span-2'>{children}</dd>
    </div>
  )
}

export function PageDeploymentGeneral({ deployment, isLoading }: Props) {
  if (isLoading || !deployment) {
    return <Skeleton className='h-64 w-full' />
  }

  return (
    <SettingsPage title='General' description='What this deployment is, and where it lives.'>
      <dl className='rounded-lg border px-5 py-1'>
        <Field label='Name'>{deployment.name}</Field>
        <Field label='Product'>{KIND_LABELS[deployment.kind]}</Field>
        <Field label='Version'>
          <span className='font-mono'>{deployment.version}</span>
        </Field>
        <Field label='Environment'>{environmentOf(deployment.namespace)}</Field>
        <Field label='Namespace'>
          <span className='font-mono text-xs'>{deployment.namespace}</span>
        </Field>
        <Field label='Created'>{format(new Date(deployment.created_at), 'd MMMM yyyy')}</Field>
        <Field label='Identifier'>
          <span className='font-mono text-xs text-muted-foreground'>{deployment.id}</span>
        </Field>
      </dl>
    </SettingsPage>
  )
}
