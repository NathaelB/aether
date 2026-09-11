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
import { RISK_LABELS } from '../../status'
import { EMPTY_PUBLISH_FORM, validatePublish, type PublishForm } from '../../publish'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  kind: Schemas.DeploymentKind
  onPublish: (request: Schemas.PublishReleaseRequest) => void
  isPublishing: boolean
}

function Field({
  label,
  hint,
  error,
  children,
}: {
  label: string
  hint?: string
  error?: string
  children: React.ReactNode
}) {
  return (
    <div className='space-y-1.5'>
      <Label>{label}</Label>
      {children}
      {error ? (
        <p className='text-xs text-destructive'>{error}</p>
      ) : (
        hint && <p className='text-xs text-muted-foreground'>{hint}</p>
      )}
    </div>
  )
}

export function PublishReleaseSheet({
  open,
  onOpenChange,
  kind,
  onPublish,
  isPublishing,
}: Props) {
  const [form, setForm] = useState<PublishForm>(EMPTY_PUBLISH_FORM)
  const [errors, setErrors] = useState<Partial<Record<keyof PublishForm, string>>>({})

  const set = (field: keyof PublishForm, value: string) =>
    setForm((current) => ({ ...current, [field]: value }))

  const submit = () => {
    const result = validatePublish(form)

    if ('errors' in result) {
      setErrors(result.errors)
      return
    }

    setErrors({})
    onPublish(result.request)
    setForm(EMPTY_PUBLISH_FORM)
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className='w-full overflow-y-auto sm:max-w-lg'>
        <SheetHeader>
          <SheetTitle>Publish a version</SheetTitle>
          <SheetDescription>
            It arrives as planned, visible to nobody but you, until you make it available.
          </SheetDescription>
        </SheetHeader>

        <div className='space-y-5 px-4 pb-6'>
          <Field
            label='Version'
            hint='Exact, as the product publishes it. Not a tag such as latest.'
            error={errors.version}
          >
            <Input
              value={form.version}
              onChange={(event) => set('version', event.target.value)}
              placeholder='26.0.1'
              autoComplete='off'
            />
          </Field>

          <Field label='Risk' hint='What a customer should expect before clicking upgrade.'>
            <Select
              value={form.risk}
              onValueChange={(value) => set('risk', value as Schemas.BreakingRisk)}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(Object.keys(RISK_LABELS) as Schemas.BreakingRisk[]).map((value) => (
                  <SelectItem key={value} value={value}>
                    {RISK_LABELS[value]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>

          <Field
            label='Release notes'
            hint='Read on the customer’s own screen, so write for them rather than linking away.'
          >
            <textarea
              value={form.notes}
              onChange={(event) => set('notes', event.target.value)}
              rows={6}
              className='w-full rounded-md border bg-transparent px-3 py-2 text-sm outline-none focus-visible:ring-1 focus-visible:ring-ring'
              placeholder='What changed, and anything that needs attention.'
            />
          </Field>

          <Field
            label='Steps through'
            hint='Versions that must be passed through to reach this one, comma separated. Leave empty if it can be reached in one hop.'
            error={errors.stepsThrough}
          >
            <Input
              value={form.stepsThrough}
              onChange={(event) => set('stepsThrough', event.target.value)}
              placeholder='25.0.0, 26.0.0'
              autoComplete='off'
            />
          </Field>

          <Field
            label='Minimum operator version'
            hint='Held back from clusters running an older data plane. Leave empty if any can take it.'
            error={errors.minimumOperatorVersion}
          >
            <Input
              value={form.minimumOperatorVersion}
              onChange={(event) => set('minimumOperatorVersion', event.target.value)}
              placeholder='0.1.0'
              autoComplete='off'
            />
          </Field>

          <div className='flex justify-end gap-2 pt-2'>
            <Button variant='outline' size='sm' onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button size='sm' onClick={submit} disabled={isPublishing}>
              Publish for {kind === 'ferriskey' ? 'FerrisKey' : 'Keycloak'}
            </Button>
          </div>
        </div>
      </SheetContent>
    </Sheet>
  )
}
