import { CheckCircle2, CircleAlert, Link2Off } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'

/** What the screen has to say, and nothing else. */
export type AcceptOutcome =
  | { kind: 'no-token' }
  | { kind: 'working' }
  | { kind: 'joined'; organisation: string }
  | { kind: 'refused'; reason: string }

interface Props {
  outcome: AcceptOutcome
  onOpen: (organisation: string) => void
  onGoHome: () => void
}

export function PageAcceptInvitation({ outcome, onOpen, onGoHome }: Props) {
  return (
    <div className='mx-auto flex min-h-[60vh] max-w-md flex-col items-center justify-center gap-4 text-center'>
      {outcome.kind === 'working' && (
        <>
          <Spinner className='size-6' />
          <p className='text-sm text-muted-foreground'>Taking you in…</p>
        </>
      )}

      {outcome.kind === 'joined' && (
        <>
          <Icon tone='ok'>
            <CheckCircle2 className='h-6 w-6' />
          </Icon>
          <div className='space-y-1'>
            <h1 className='text-lg font-semibold'>You are in</h1>
            <p className='text-sm text-muted-foreground'>
              The invitation has been used and cannot be used again.
            </p>
          </div>
          <Button onClick={() => onOpen(outcome.organisation)}>Open the organisation</Button>
        </>
      )}

      {outcome.kind === 'refused' && (
        <>
          <Icon tone='bad'>
            <CircleAlert className='h-6 w-6' />
          </Icon>
          <div className='space-y-1'>
            <h1 className='text-lg font-semibold'>This link did not work</h1>
            {/* The platform's own sentence. It already distinguishes expired
                from unknown from addressed-to-somebody-else, and rewriting it
                here would lose that. */}
            <p className='text-sm text-muted-foreground'>{outcome.reason}</p>
          </div>
          <Button variant='outline' onClick={onGoHome}>
            Go to the console
          </Button>
        </>
      )}

      {outcome.kind === 'no-token' && (
        <>
          <Icon tone='bad'>
            <Link2Off className='h-6 w-6' />
          </Icon>
          <div className='space-y-1'>
            <h1 className='text-lg font-semibold'>There is no invitation in this link</h1>
            <p className='text-sm text-muted-foreground'>
              It was probably cut short on the way. Ask whoever invited you for the whole thing.
            </p>
          </div>
          <Button variant='outline' onClick={onGoHome}>
            Go to the console
          </Button>
        </>
      )}
    </div>
  )
}

function Icon({ tone, children }: { tone: 'ok' | 'bad'; children: React.ReactNode }) {
  return (
    <div
      className={
        tone === 'ok'
          ? 'flex h-12 w-12 items-center justify-center rounded-lg border bg-background text-foreground'
          : 'flex h-12 w-12 items-center justify-center rounded-lg border bg-background text-destructive'
      }
    >
      {children}
    </div>
  )
}
