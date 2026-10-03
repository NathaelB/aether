import { useState } from 'react'
import { Palette } from 'lucide-react'
import type { Schemas } from '@/api/api.client'
import { EmptyState, Section, SectionPage } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { StatusBadge } from '@/components/ui/status-badge'
import { cn } from '@/lib/utils'
import {
  COLOR_FIELDS,
  COLOR_LABELS,
  DEFAULT_COLORS,
  MAX_RADIUS,
  MIN_RADIUS,
  describeColorProblem,
  describeRadiusProblem,
  describeUnavailable,
  hasStoredBranding,
  isDirty,
  isHex,
  isValid,
  normaliseHex,
  previewOf,
  requestLabel,
  REQUEST_NOTE,
  toForm,
  type BrandingForm,
  type ColorField,
} from '../../branding'
import { BrandingPreview } from './components/branding-preview'

interface Props {
  isLoading: boolean
  feature?: Schemas.IamFeatureAvailability
  state?: Schemas.IamSettingsState
  onApply: (form: BrandingForm) => void
  onReset: () => void
  isSaving: boolean
  error?: string
}

export function PageBranding({
  isLoading,
  feature,
  state,
  onApply,
  onReset,
  isSaving,
  error,
}: Props) {
  if (isLoading) {
    return <Skeleton className='h-64 w-full' />
  }

  if (!feature?.open) {
    return (
      <SectionPage
        title='Branding'
        description='Colors and corner radius of the login portal of this instance.'
      >
        <EmptyState
          icon={<Palette className='h-5 w-5' />}
          title='Branding is not included in your plan'
          description={describeUnavailable(feature?.opened_by)}
        />
      </SectionPage>
    )
  }

  const stored = state?.branding ?? null

  return (
    <SectionPage
      title='Branding'
      description='Colors and corner radius of the login portal of this instance.'
    >
      <BrandingEditor
        key={JSON.stringify(toForm(stored))}
        stored={stored}
        request={state?.request ?? null}
        onApply={onApply}
        onReset={onReset}
        isSaving={isSaving}
        error={error}
      />
    </SectionPage>
  )
}

function BrandingEditor({
  stored,
  request,
  onApply,
  onReset,
  isSaving,
  error,
}: {
  stored: Schemas.Branding | null
  request: Schemas.IamSettingsRequest | null
  onApply: (form: BrandingForm) => void
  onReset: () => void
  isSaving: boolean
  error?: string
}) {
  const [form, setForm] = useState<BrandingForm>(() => toForm(stored))
  const [resetting, setResetting] = useState(false)

  const canApply = isDirty(form, stored) && isValid(form) && !isSaving
  const setColor = (field: ColorField, value: string) =>
    setForm((current) => ({ ...current, colors: { ...current.colors, [field]: value } }))
  const radiusProblem = describeRadiusProblem(form.radius)

  return (
    <>
      <RequestState request={request} />

      <Section
        title='Login portal'
        aside={
          <div className='flex items-center gap-2'>
            <Button
              variant='outline'
              size='sm'
              disabled={isSaving || !hasStoredBranding(stored)}
              onClick={() => setResetting(true)}
            >
              Reset to defaults
            </Button>
            <Button size='sm' disabled={!canApply} onClick={() => onApply(form)}>
              {isSaving && <Spinner className='size-3.5' />}
              {isSaving ? 'Applying' : 'Apply'}
            </Button>
          </div>
        }
      >
        {error && (
          <p role='alert' className='text-sm text-destructive'>
            {error}
          </p>
        )}

        <div className='grid gap-6 lg:grid-cols-2'>
          <div className='space-y-4 rounded-lg border p-5'>
            {COLOR_FIELDS.map((field) => (
              <ColorRow
                key={field}
                field={field}
                value={form.colors[field]}
                onChange={(value) => setColor(field, value)}
              />
            ))}

            <div className='space-y-2'>
              <Label htmlFor='branding-radius'>Corner radius</Label>
              <Input
                id='branding-radius'
                type='number'
                min={MIN_RADIUS}
                max={MAX_RADIUS}
                step={1}
                value={form.radius}
                onChange={(event) => setForm({ ...form, radius: event.target.value })}
                placeholder='3'
                aria-invalid={!!radiusProblem}
                className={cn('w-28', radiusProblem && 'border-destructive')}
              />
              {radiusProblem ? (
                <p className='text-xs text-destructive'>{radiusProblem}</p>
              ) : (
                <p className='text-xs text-muted-foreground'>
                  From {MIN_RADIUS} to {MAX_RADIUS}. Leave empty for FerrisKey's default.
                </p>
              )}
            </div>
          </div>

          <div className='space-y-2'>
            <p className='text-sm font-medium'>Preview</p>
            <BrandingPreview preview={previewOf(form)} />
          </div>
        </div>
      </Section>

      <ResetDialog
        open={resetting}
        onOpenChange={setResetting}
        isSaving={isSaving}
        onConfirm={() => {
          onReset()
          setResetting(false)
        }}
      />
    </>
  )
}

function ColorRow({
  field,
  value,
  onChange,
}: {
  field: ColorField
  value: string
  onChange: (value: string) => void
}) {
  const id = `branding-${field}`
  const problem = describeColorProblem(value)
  const fallback = DEFAULT_COLORS[field]

  return (
    <div className='space-y-1.5'>
      <Label htmlFor={id}>{COLOR_LABELS[field]}</Label>
      <div className='flex items-center gap-2'>
        <input
          type='color'
          aria-label={`${COLOR_LABELS[field]} picker`}
          value={isHex(value.trim()) ? normaliseHex(value) : fallback}
          onChange={(event) => onChange(event.target.value)}
          className='h-9 w-12 cursor-pointer rounded-md border bg-transparent p-1'
        />
        <Input
          id={id}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder={fallback}
          aria-invalid={!!problem}
          aria-describedby={problem ? `${id}-problem` : undefined}
          className={cn('w-40 font-mono', problem && 'border-destructive')}
        />
      </div>
      {problem && (
        <p id={`${id}-problem`} className='text-xs text-destructive'>
          {problem}
        </p>
      )}
    </div>
  )
}

function RequestState({ request }: { request: Schemas.IamSettingsRequest | null }) {
  if (!request) return null
  const { label, tone } = requestLabel(request.status)

  return (
    <div className='overflow-hidden rounded-lg border'>
      <div className='flex items-center gap-2 border-b bg-muted/30 px-4 py-2.5'>
        <StatusBadge tone={tone}>{label}</StatusBadge>
        <span className='text-xs text-muted-foreground'>
          {new Date(request.created_at).toLocaleString()}
        </span>
      </div>
      <p className='px-4 py-2.5 text-xs text-muted-foreground'>{REQUEST_NOTE}</p>
    </div>
  )
}

function ResetDialog({
  open,
  onOpenChange,
  isSaving,
  onConfirm,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  isSaving: boolean
  onConfirm: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Reset branding</DialogTitle>
          <DialogDescription>
            The login portal goes back to FerrisKey's default colors and radius.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant='outline' onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant='destructive' onClick={onConfirm} disabled={isSaving}>
            Reset
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
