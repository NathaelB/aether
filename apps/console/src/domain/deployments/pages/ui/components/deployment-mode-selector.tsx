import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { Check, Server, Users } from 'lucide-react'
import type { DeploymentMode } from '../../../types/deployment'

interface Props {
  mode: DeploymentMode
  onSelect: (mode: DeploymentMode) => void
}

const MODES: {
  value: DeploymentMode
  label: string
  description: string
  detail: string
  icon: typeof Users
}[] = [
  {
    value: 'shared',
    label: 'Shared',
    description: 'Runs on infrastructure shared with other organisations.',
    detail: 'Placed on an existing cluster with room. Available immediately.',
    icon: Users,
  },
  {
    value: 'dedicated',
    label: 'Dedicated',
    description: 'Runs on a Kubernetes cluster of this organisation\'s own.',
    // Worth saying plainly rather than discovering it from a spinner: a
    // dedicated deployment waits for a cluster to exist and report in, so it
    // sits in Pending for longer than a shared one ever does.
    detail: 'A cluster is provisioned if the organisation has none. Takes longer to start.',
    icon: Server,
  },
]

export function DeploymentModeSelector({ mode, onSelect }: Props) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className='text-lg'>Isolation</CardTitle>
        <CardDescription>Choose what this deployment shares with others.</CardDescription>
      </CardHeader>
      <CardContent className='grid gap-3 sm:grid-cols-2'>
        {MODES.map(({ value, label, description, detail, icon: Icon }) => {
          const selected = mode === value

          return (
            <button
              key={value}
              type='button'
              onClick={() => onSelect(value)}
              aria-pressed={selected}
              className={cn(
                'relative flex flex-col gap-1 rounded-lg border p-4 text-left transition-colors',
                'hover:border-primary/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
                selected ? 'border-primary bg-primary/5' : 'border-border',
              )}
            >
              {selected && (
                <Check
                  className='absolute right-3 top-3 h-4 w-4 text-primary'
                  aria-hidden='true'
                />
              )}
              <span className='flex items-center gap-2 font-medium'>
                <Icon className='h-4 w-4 text-muted-foreground' aria-hidden='true' />
                {label}
              </span>
              <span className='text-sm text-muted-foreground'>{description}</span>
              <span className='mt-1 text-xs text-muted-foreground'>{detail}</span>
            </button>
          )
        })}
      </CardContent>
    </Card>
  )
}
