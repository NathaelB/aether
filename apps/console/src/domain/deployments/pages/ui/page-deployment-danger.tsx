import type { Schemas } from '@/api/api.client'
import { SettingsPage } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { useState } from 'react'

interface Props {
  deployment?: Schemas.Deployment
  isLoading: boolean
  onDelete: () => void
}

export function PageDeploymentDanger({ deployment, isLoading, onDelete }: Props) {
  const [typed, setTyped] = useState('')

  if (isLoading || !deployment) {
    return <Skeleton className='h-40 w-full' />
  }

  const goingAway = deployment.status === 'deleting' || deployment.status === 'deleted'
  // Typed rather than confirmed in a dialog. A dialog is dismissed by reflex;
  // writing the name back is the one gesture that cannot be made by accident.
  const confirmed = typed.trim() === deployment.name

  return (
    <SettingsPage title='Danger zone' description='Things that cannot be undone.'>
      <div className='space-y-4 rounded-lg border border-destructive/40 p-5'>
        <div className='space-y-1'>
          <p className='font-medium'>Delete this deployment</p>
          <p className='text-sm text-muted-foreground'>
            The instance and its database go with it. There is no restore: what it held is gone
            when the data plane confirms the deletion.
          </p>
        </div>

        <div className='space-y-2'>
          <Label htmlFor='confirm'>
            Type <span className='font-mono'>{deployment.name}</span> to confirm
          </Label>
          <Input
            id='confirm'
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
            className='w-full sm:w-80'
            autoComplete='off'
          />
        </div>

        <Button variant='destructive' size='sm' disabled={!confirmed || goingAway} onClick={onDelete}>
          {goingAway ? 'Already going away' : 'Delete deployment'}
        </Button>
      </div>
    </SettingsPage>
  )
}
