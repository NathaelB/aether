import { describe, expect, it } from 'vitest'
import {
  DEFAULT_COLORS,
  DEFAULT_RADIUS,
  DEFAULT_WIDGET_RADIUS,
  describeColorProblem,
  describeRadiusProblem,
  describeUnavailable,
  emptyForm,
  hasStoredBranding,
  isDirty,
  isHex,
  isValid,
  normaliseHex,
  previewOf,
  requestLabel,
  toApplyRequest,
  toBranding,
  toForm,
  toResetRequest,
} from './branding'

describe('hex colors', () => {
  it('accepts only #rrggbb', () => {
    for (const ok of ['#635dff', '#FFFFFF', '#000000']) expect(isHex(ok), ok).toBe(true)
    for (const bad of ['', '635dff', '#fff', '#12345g', '#1234567', ' #635dff'])
      expect(isHex(bad), bad).toBe(false)
  })

  it('normalises case and spaces', () => {
    expect(normaliseHex(' #ABCDEF ')).toBe('#abcdef')
  })

  it('treats empty as fine and a bad value as a problem', () => {
    expect(describeColorProblem('')).toBeNull()
    expect(describeColorProblem('#635dff')).toBeNull()
    expect(describeColorProblem('red')).toMatch(/#rrggbb/)
  })
})

describe('radius', () => {
  it('accepts empty and 0 to 24', () => {
    for (const ok of ['', '0', '12', '24']) expect(describeRadiusProblem(ok), ok).toBeNull()
  })

  it('refuses the rest', () => {
    for (const bad of ['-1', '25', '1.5', 'abc'])
      expect(describeRadiusProblem(bad), bad).not.toBeNull()
  })
})

describe('request body', () => {
  it('sends only the filled colors, lowercased', () => {
    const form = emptyForm()
    form.colors.primary = '#ABCDEF'
    expect(toApplyRequest(form)).toEqual({ branding: { colors: { primary: '#abcdef' } } })
  })

  it('omits the radius when empty and keeps 0', () => {
    const form = emptyForm()
    expect(toBranding(form)).toEqual({ colors: {} })
    form.radius = '0'
    expect(toBranding(form)).toEqual({ colors: {}, radius: 0 })
  })

  it('resets with null', () => {
    expect(toResetRequest()).toEqual({ branding: null })
  })
})

describe('form and stored value', () => {
  it('round trips', () => {
    const stored = { colors: { primary: '#111111', error: '#222222' }, radius: 8 }
    expect(toBranding(toForm(stored))).toEqual(stored)
  })

  it('reads nothing as an empty form', () => {
    expect(toForm(null)).toEqual(emptyForm())
    expect(toForm(undefined)).toEqual(emptyForm())
  })

  it('is not dirty when nothing changed, ignoring case', () => {
    const stored = { colors: { primary: '#aabbcc' }, radius: 4 }
    const form = toForm(stored)
    expect(isDirty(form, stored)).toBe(false)
    form.colors.primary = '#AABBCC'
    expect(isDirty(form, stored)).toBe(false)
  })

  it('is dirty after an edit', () => {
    const stored = { colors: { primary: '#aabbcc' }, radius: 4 }
    const form = toForm(stored)
    form.radius = '5'
    expect(isDirty(form, stored)).toBe(true)
    expect(isDirty(toForm(stored), null)).toBe(true)
    expect(isDirty(emptyForm(), null)).toBe(false)
  })

  it('knows whether something is stored', () => {
    expect(hasStoredBranding(null)).toBe(false)
    expect(hasStoredBranding({ colors: {}, radius: null })).toBe(false)
    expect(hasStoredBranding({ radius: 0 })).toBe(true)
  })

  it('is invalid with a bad color or radius', () => {
    const form = emptyForm()
    expect(isValid(form)).toBe(true)
    form.colors.text = '#12'
    expect(isValid(form)).toBe(false)
    form.colors.text = ''
    form.radius = '99'
    expect(isValid(form)).toBe(false)
  })
})

describe('preview', () => {
  it('falls back to the defaults', () => {
    expect(previewOf(emptyForm())).toEqual({
      colors: DEFAULT_COLORS,
      radius: DEFAULT_RADIUS,
      widgetRadius: DEFAULT_WIDGET_RADIUS,
    })
  })

  it('uses the valid entered values and ignores invalid ones', () => {
    const form = emptyForm()
    form.colors.primary = '#ABCDEF'
    form.colors.links = 'nope'
    form.radius = '0'
    const preview = previewOf(form)
    expect(preview.colors.primary).toBe('#abcdef')
    expect(preview.colors.links).toBe(DEFAULT_COLORS.links)
    expect(preview.radius).toBe(0)
    expect(preview.widgetRadius).toBe(0)
    form.radius = '40'
    expect(previewOf(form).radius).toBe(DEFAULT_RADIUS)
    expect(previewOf(form).widgetRadius).toBe(DEFAULT_WIDGET_RADIUS)
  })
})

describe('request status', () => {
  it('labels each state', () => {
    expect(requestLabel('Pending')).toEqual({ label: 'Requested', tone: 'neutral' })
    expect(requestLabel({ Published: { at: 'x' } })).toEqual({
      label: 'Handed to the data plane',
      tone: 'success',
    })
    expect(requestLabel({ Leased: { until: 'x' } }).tone).toBe('progress')
    expect(requestLabel({ Pulled: { agent_id: 'a', at: 'x' } }).tone).toBe('progress')
    expect(requestLabel({ Failed: { at: 'x', reason: 'Expired' } } as never)).toEqual({
      label: 'Failed',
      tone: 'danger',
    })
  })
})

describe('gate message', () => {
  it('names the plan when given', () => {
    expect(describeUnavailable('Business')).toContain('Business')
    expect(describeUnavailable(null)).not.toContain('from the')
  })
})
