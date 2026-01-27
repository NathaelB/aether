import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

interface PageCreateOrganisationProps {
  name: string
  slug: string
  isSubmitting?: boolean
  error?: string | null
  onNameChange: (value: string) => void
  onSlugChange: (value: string) => void
  onSubmit: () => void
}

export function PageCreateOrganisation({
  name,
  slug,
  isSubmitting = false,
  error = null,
  onNameChange,
  onSlugChange,
  onSubmit,
}: PageCreateOrganisationProps) {
  return (
    <div className='relative mx-auto flex w-full max-w-5xl flex-col gap-8'>
      <div className='absolute -top-16 right-10 h-48 w-48 rounded-full bg-[radial-gradient(circle_at_center,rgba(59,130,246,0.18),transparent_65%)] blur-2xl' />
      <div className='absolute -bottom-24 left-6 h-56 w-56 rounded-full bg-[radial-gradient(circle_at_center,rgba(16,185,129,0.18),transparent_65%)] blur-2xl' />

      <div className='flex flex-col gap-6 rounded-2xl border bg-background/60 p-6 backdrop-blur md:p-8'>
        <div className='flex flex-col gap-3'>
          <div className='inline-flex w-fit items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium text-muted-foreground'>
            Step 1 of 2
          </div>
          <h1 className='text-3xl font-semibold tracking-tight text-foreground'>
            Create your organisation
          </h1>
          <p className='max-w-2xl text-sm text-muted-foreground'>
            Your organisation is the home for deployments, environments, and collaborators. You can change the
            name and slug later.
          </p>
        </div>

        <div className='grid gap-6 lg:grid-cols-[1.2fr_0.8fr]'>
          <Card className='border-muted/70'>
            <CardHeader>
              <CardTitle>Organisation details</CardTitle>
              <CardDescription>Pick a name people recognize and a short URL-friendly slug.</CardDescription>
            </CardHeader>
            <CardContent className='space-y-5'>
              <div className='space-y-2'>
                <Label htmlFor='org-name'>Name</Label>
                <Input
                  id='org-name'
                  placeholder='Acme Labs'
                  value={name}
                  onChange={(event) => onNameChange(event.target.value)}
                />
              </div>
              <div className='space-y-2'>
                <Label htmlFor='org-slug'>Slug</Label>
                <Input
                  id='org-slug'
                  placeholder='acme'
                  value={slug}
                  onChange={(event) => onSlugChange(event.target.value)}
                />
              </div>
              {error ? (
                <div className='rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive'>
                  {error}
                </div>
              ) : null}
              <div className='flex flex-col gap-2 text-xs text-muted-foreground'>
                <div className='flex items-center gap-2'>
                  <span className='h-1.5 w-1.5 rounded-full bg-emerald-500' />
                  Slug is used in URLs and API paths.
                </div>
                <div className='flex items-center gap-2'>
                  <span className='h-1.5 w-1.5 rounded-full bg-sky-500' />
                  You can invite teammates after this step.
                </div>
              </div>
              <div className='flex items-center justify-between gap-3 pt-2'>
                <Button type='button' variant='ghost'>
                  Back
                </Button>
                <Button type='button' onClick={onSubmit} disabled={isSubmitting || !name.trim()}>
                  {isSubmitting ? 'Creating…' : 'Create organisation'}
                </Button>
              </div>
            </CardContent>
          </Card>

          <div className='flex h-full flex-col gap-4'>
            <Card className='border-muted/70'>
              <CardHeader>
                <CardTitle>What you get</CardTitle>
                <CardDescription>Everything you need to start shipping.</CardDescription>
              </CardHeader>
              <CardContent className='space-y-4 text-sm text-muted-foreground'>
                <div className='flex items-start gap-3'>
                  <div className='mt-1 h-2 w-2 rounded-full bg-foreground' />
                  <div>
                    <div className='text-foreground'>Unified deployments</div>
                    Manage infra and identity providers from one place.
                  </div>
                </div>
                <div className='flex items-start gap-3'>
                  <div className='mt-1 h-2 w-2 rounded-full bg-foreground' />
                  <div>
                    <div className='text-foreground'>Audit-ready activity</div>
                    Keep track of every action with timeline visibility.
                  </div>
                </div>
                <div className='flex items-start gap-3'>
                  <div className='mt-1 h-2 w-2 rounded-full bg-foreground' />
                  <div>
                    <div className='text-foreground'>Team-ready access</div>
                    Add members and assign roles in seconds.
                  </div>
                </div>
              </CardContent>
            </Card>

            <div className='rounded-2xl border border-dashed bg-muted/30 p-4 text-xs text-muted-foreground'>
              Tip: Use a short slug, like your company or project name.
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
