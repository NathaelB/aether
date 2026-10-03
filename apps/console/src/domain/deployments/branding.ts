import type { Schemas } from '@/api/api.client'

export const COLOR_FIELDS = [
  'primary',
  'primary_text',
  'links',
  'page_background',
  'widget_background',
  'text',
  'error',
] as const

export type ColorField = (typeof COLOR_FIELDS)[number]

export const COLOR_LABELS: Record<ColorField, string> = {
  primary: 'Primary button',
  primary_text: 'Button label',
  links: 'Links',
  page_background: 'Page background',
  widget_background: 'Card background',
  text: 'Text',
  error: 'Error',
}

export const DEFAULT_COLORS: Record<ColorField, string> = {
  primary: '#635dff',
  primary_text: '#ffffff',
  links: '#635dff',
  page_background: '#000000',
  widget_background: '#ffffff',
  text: '#1e212a',
  error: '#d03c38',
}

export const MIN_RADIUS = 0
export const MAX_RADIUS = 24
export const DEFAULT_RADIUS = 3
export const DEFAULT_WIDGET_RADIUS = 5

export interface BrandingForm {
  colors: Record<ColorField, string>
  radius: string
}

export interface StatusLabel {
  label: string
  tone: 'neutral' | 'progress' | 'success' | 'danger'
}

export type Branding = Schemas.Branding

const HEX = /^#[0-9a-fA-F]{6}$/

export function isHex(value: string): boolean {
  return HEX.test(value)
}

export function normaliseHex(value: string): string {
  return value.trim().toLowerCase()
}

export function describeColorProblem(value: string): string | null {
  const trimmed = value.trim()
  if (trimmed === '' || isHex(trimmed)) return null
  return 'Use a hex color written as #rrggbb, for example #635dff.'
}

export function describeRadiusProblem(value: string): string | null {
  const trimmed = value.trim()
  if (trimmed === '') return null
  const radius = Number(trimmed)
  if (!Number.isInteger(radius) || radius < MIN_RADIUS || radius > MAX_RADIUS) {
    return `The radius is a whole number from ${MIN_RADIUS} to ${MAX_RADIUS}.`
  }
  return null
}

export function emptyForm(): BrandingForm {
  return {
    colors: {
      primary: '',
      primary_text: '',
      links: '',
      page_background: '',
      widget_background: '',
      text: '',
      error: '',
    },
    radius: '',
  }
}

export function toForm(branding: Branding | null | undefined): BrandingForm {
  const form = emptyForm()
  for (const field of COLOR_FIELDS) {
    form.colors[field] = branding?.colors?.[field] ?? ''
  }
  form.radius = branding?.radius == null ? '' : String(branding.radius)
  return form
}

export function isValid(form: BrandingForm): boolean {
  return (
    COLOR_FIELDS.every((field) => describeColorProblem(form.colors[field]) === null) &&
    describeRadiusProblem(form.radius) === null
  )
}

export function toBranding(form: BrandingForm): Schemas.BrandingInput {
  const colors: Schemas.BrandingColorsInput = {}
  for (const field of COLOR_FIELDS) {
    const value = normaliseHex(form.colors[field])
    if (value !== '') colors[field] = value
  }

  const branding: Schemas.BrandingInput = { colors }
  if (form.radius.trim() !== '') branding.radius = Number(form.radius.trim())
  return branding
}

export function toApplyRequest(form: BrandingForm): Schemas.SetIamSettingsRequest {
  return { branding: toBranding(form) }
}

export function toResetRequest(): Schemas.SetIamSettingsRequest {
  return { branding: null }
}

export function isDirty(form: BrandingForm, stored: Branding | null | undefined): boolean {
  return JSON.stringify(toBranding(form)) !== JSON.stringify(toBranding(toForm(stored)))
}

export function hasStoredBranding(stored: Branding | null | undefined): boolean {
  const form = toForm(stored)
  return COLOR_FIELDS.some((field) => form.colors[field] !== '') || form.radius !== ''
}

export interface PreviewStyle {
  colors: Record<ColorField, string>
  radius: number
  widgetRadius: number
}

export function previewOf(form: BrandingForm): PreviewStyle {
  const colors = { ...DEFAULT_COLORS }
  for (const field of COLOR_FIELDS) {
    const value = form.colors[field].trim()
    if (isHex(value)) colors[field] = value.toLowerCase()
  }
  const radius = Number(form.radius.trim())
  const usable = form.radius.trim() !== '' && describeRadiusProblem(form.radius) === null
  return {
    colors,
    radius: usable ? radius : DEFAULT_RADIUS,
    widgetRadius: usable ? radius : DEFAULT_WIDGET_RADIUS,
  }
}

export function requestLabel(status: Schemas.ActionStatus): StatusLabel {
  if (status === 'Pending') return { label: 'Requested', tone: 'neutral' }
  if ('Leased' in status) return { label: 'Being picked up', tone: 'progress' }
  if ('Pulled' in status) return { label: 'Being picked up', tone: 'progress' }
  if ('Published' in status) return { label: 'Handed to the data plane', tone: 'success' }
  return { label: 'Failed', tone: 'danger' }
}

export const REQUEST_NOTE =
  'This is the state of the request, not proof that the instance applied it.'

export function describeUnavailable(opensWith: Schemas.Plan | null | undefined): string {
  return opensWith
    ? `Branding is not included in your organisation's plan. It is available from the ${opensWith} plan.`
    : 'Branding is not included in your organisation\'s plan.'
}
