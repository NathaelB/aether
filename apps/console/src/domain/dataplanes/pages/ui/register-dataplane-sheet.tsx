import { useState } from 'react'
import type { Schemas } from '@/api/api.client'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { toCreateRequest, whatIsMissing, type RegistrationForm } from '../../registration'
import { HeraldCredential } from './components/herald-credential'

/** A machine somebody has: four cores, eight gigabytes, a disk. */
const EMPTY_FORM: RegistrationForm = {
  region: '',
  mode: 'shared',
  organisationId: null,
  capacity: { vcpu: '4', memoryGib: '8', storageGib: '100' },
}

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  organisations: Schemas.Tenant[]
  onRegister: (request: Schemas.CreateDataPlaneRequest) => void
  isRegistering: boolean
  /** Present once the control plane has answered, and only then. */
  registered?: {
    dataplaneId: string
    clientId: string
    clientSecret: string
  }
  refusal?: string
  apiUrl: string
  issuerUrl: string
}

export function RegisterDataPlaneSheet({
  open,
  onOpenChange,
  organisations,
  onRegister,
  isRegistering,
  registered,
  refusal,
  apiUrl,
  issuerUrl,
}: Props) {
  const [form, setForm] = useState<RegistrationForm>(EMPTY_FORM)
  const [submitted, setSubmitted] = useState(false)

  const missing = whatIsMissing(form)

  return (
    <Sheet
      open={open}
      onOpenChange={(next) => {
        if (!next) {
          setForm(EMPTY_FORM)
          setSubmitted(false)
        }
        onOpenChange(next)
      }}
    >
      <SheetContent className='w-full overflow-y-auto sm:max-w-xl'>
        <SheetHeader>
          <SheetTitle>{registered ? 'Install it' : 'Register a data plane'}</SheetTitle>
          <SheetDescription>
            {registered
              ? 'The cluster is registered. It becomes a placement candidate once its Herald reports.'
              : 'A cluster you already run. Register it here, install the chart there, and it announces itself.'}
          </SheetDescription>
        </SheetHeader>

        <div className='px-4 pb-6'>
          {registered ? (
            <HeraldCredential
              dataplaneId={registered.dataplaneId}
              clientId={registered.clientId}
              clientSecret={registered.clientSecret}
              apiUrl={apiUrl}
              issuerUrl={issuerUrl}
              onDone={() => onOpenChange(false)}
            />
          ) : (
            <form
              className='space-y-5'
              onSubmit={(event) => {
                event.preventDefault()
                setSubmitted(true)

                const request = toCreateRequest(form)
                if (request) onRegister(request)
              }}
            >
              <div className='space-y-2'>
                <Label htmlFor='region'>Region</Label>
                <Input
                  id='region'
                  value={form.region}
                  onChange={(event) => setForm({ ...form, region: event.target.value })}
                  placeholder='fr-par'
                />
                <p className='text-xs text-muted-foreground'>
                  What this cluster serves, in your own words. Customers never choose one; it is
                  how you decide where a deployment lands.
                </p>
              </div>

              <div className='space-y-2'>
                <Label htmlFor='mode'>Open to</Label>
                <Select
                  value={form.mode}
                  onValueChange={(mode) =>
                    setForm({ ...form, mode: mode as Schemas.DataPlaneMode })
                  }
                >
                  <SelectTrigger id='mode' className='w-full'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='shared'>Any organisation with room</SelectItem>
                    <SelectItem value='dedicated'>One organisation</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              {form.mode === 'dedicated' && (
                <div className='space-y-2'>
                  <Label htmlFor='organisation'>Organisation</Label>
                  <Select
                    value={form.organisationId ?? ''}
                    onValueChange={(organisationId) => setForm({ ...form, organisationId })}
                  >
                    <SelectTrigger id='organisation' className='w-full'>
                      <SelectValue placeholder='Whose cluster is this?' />
                    </SelectTrigger>
                    <SelectContent>
                      {organisations.map(({ organisation }) => (
                        <SelectItem key={organisation.id} value={organisation.id}>
                          {organisation.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              )}

              <fieldset className='space-y-2'>
                <legend className='text-sm font-medium'>Capacity</legend>
                <p className='text-xs text-muted-foreground'>
                  What this machine may give out. Placement adds up what is on it and stops here,
                  so size it to the machine rather than to its hardware.
                </p>
                <div className='grid gap-3 sm:grid-cols-3'>
                  <div className='space-y-1.5'>
                    <Label htmlFor='vcpu' className='text-xs'>
                      vCPU
                    </Label>
                    <Input
                      id='vcpu'
                      value={form.capacity.vcpu}
                      onChange={(event) =>
                        setForm({
                          ...form,
                          capacity: { ...form.capacity, vcpu: event.target.value },
                        })
                      }
                    />
                  </div>
                  <div className='space-y-1.5'>
                    <Label htmlFor='memory' className='text-xs'>
                      Memory (GiB)
                    </Label>
                    <Input
                      id='memory'
                      value={form.capacity.memoryGib}
                      onChange={(event) =>
                        setForm({
                          ...form,
                          capacity: { ...form.capacity, memoryGib: event.target.value },
                        })
                      }
                    />
                  </div>
                  <div className='space-y-1.5'>
                    <Label htmlFor='storage' className='text-xs'>
                      Storage (GiB)
                    </Label>
                    <Input
                      id='storage'
                      value={form.capacity.storageGib}
                      onChange={(event) =>
                        setForm({
                          ...form,
                          capacity: { ...form.capacity, storageGib: event.target.value },
                        })
                      }
                    />
                  </div>
                </div>
              </fieldset>

              {submitted && missing && (
                <p className='rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive'>
                  {missing}
                </p>
              )}

              {refusal && (
                <p className='rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive'>
                  {refusal}
                </p>
              )}

              <div className='flex justify-end gap-2 border-t pt-4'>
                <Button type='button' variant='ghost' onClick={() => onOpenChange(false)}>
                  Cancel
                </Button>
                <Button type='submit' disabled={isRegistering}>
                  {isRegistering ? 'Registering…' : 'Register'}
                </Button>
              </div>
            </form>
          )}
        </div>
      </SheetContent>
    </Sheet>
  )
}
