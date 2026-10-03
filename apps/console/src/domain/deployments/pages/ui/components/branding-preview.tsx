import type { PreviewStyle } from '../../../branding'

export function BrandingPreview({ preview }: { preview: PreviewStyle }) {
  const { colors, radius, widgetRadius } = preview

  return (
    <div
      className='flex items-center justify-center rounded-lg border p-8'
      style={{ backgroundColor: colors.page_background }}
      aria-label='Login preview'
    >
      <div
        className='w-full max-w-xs space-y-4 p-6 shadow-sm'
        style={{
          backgroundColor: colors.widget_background,
          color: colors.text,
          borderRadius: widgetRadius,
        }}
      >
        <p className='text-lg font-semibold'>Sign in</p>
        <input
          readOnly
          tabIndex={-1}
          placeholder='Email address'
          className='w-full border bg-transparent px-3 py-2 text-sm'
          style={{ borderRadius: radius, borderColor: colors.text }}
        />
        <p className='text-xs' style={{ color: colors.error }}>
          Wrong email or password.
        </p>
        <button
          type='button'
          tabIndex={-1}
          className='w-full px-3 py-2 text-sm font-medium'
          style={{
            backgroundColor: colors.primary,
            color: colors.primary_text,
            borderRadius: radius,
          }}
        >
          Continue
        </button>
        <p className='text-center text-xs underline' style={{ color: colors.links }}>
          Forgot your password?
        </p>
      </div>
    </div>
  )
}
