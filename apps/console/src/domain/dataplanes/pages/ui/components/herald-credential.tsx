import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Check, Copy, TriangleAlert } from 'lucide-react'
import { DEFAULT_NAMESPACE, helmCommand } from '../../../registration'

interface Props {
  dataplaneId: string
  clientId: string
  clientSecret: string
  /** What the browser talks to. A starting point, not an answer — see below. */
  apiUrl: string
  issuerUrl: string
  onDone: () => void
}

function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false)

  return (
    <Button
      type='button'
      variant='outline'
      size='sm'
      onClick={() => {
        void navigator.clipboard?.writeText(text)
        setCopied(true)
        window.setTimeout(() => setCopied(false), 2000)
      }}
    >
      {copied ? <Check className='h-3.5 w-3.5' /> : <Copy className='h-3.5 w-3.5' />}
      {copied ? 'Copied' : label}
    </Button>
  )
}

/**
 * The credential, and the command it belongs in.
 *
 * Built around one fact: the secret is minted here and stored nowhere, so
 * this is the only time it can be read. That is why the command is filled in
 * rather than described — the gap between a credential shown once and a
 * cluster that works should be one paste, not a form somebody reconstructs
 * from a secret they pasted into a notes app.
 */
export function HeraldCredential({
  dataplaneId,
  clientId,
  clientSecret,
  apiUrl,
  issuerUrl,
  onDone,
}: Props) {
  // Editable, and prefilled with what the browser uses, which is often not
  // reachable from inside a cluster: localhost is the operator's machine, not
  // the node. Getting this wrong is the difference between a Herald that
  // reports and one that never does, and nothing else in the flow would say
  // which happened.
  const [controlPlaneUrl, setControlPlaneUrl] = useState(apiUrl)
  const [namespace, setNamespace] = useState(DEFAULT_NAMESPACE)

  const command = helmCommand({
    dataplaneId,
    controlPlaneUrl,
    issuerUrl,
    clientId,
    clientSecret,
    namespace,
  })

  return (
    <div className='space-y-5'>
      <div className='flex gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200'>
        <TriangleAlert className='mt-0.5 h-4 w-4 shrink-0' />
        <p>
          This secret is not stored anywhere and will not be shown again. Install the chart now,
          or keep it somewhere safe — otherwise the way back is re-issuing, which stops the
          credential you were given here from working.
        </p>
      </div>

      <div className='space-y-2'>
        <Label htmlFor='control-plane-url'>Control plane URL</Label>
        <Input
          id='control-plane-url'
          value={controlPlaneUrl}
          onChange={(event) => setControlPlaneUrl(event.target.value)}
        />
        <p className='text-xs text-muted-foreground'>
          As reachable <em>from inside the cluster</em>, which is rarely what this browser uses.
          For a k3d cluster on this machine, that is usually{' '}
          <code className='font-mono'>http://host.k3d.internal:&lt;port&gt;</code>.
        </p>
      </div>

      <div className='space-y-2'>
        <Label htmlFor='namespace'>Namespace</Label>
        <Input
          id='namespace'
          value={namespace}
          onChange={(event) => setNamespace(event.target.value)}
        />
      </div>

      <div className='space-y-2'>
        <div className='flex items-center justify-between gap-2'>
          <Label>Install it</Label>
          <CopyButton text={command} label='Copy command' />
        </div>
        <pre className='overflow-x-auto rounded-lg border bg-muted/30 p-3 font-mono text-xs leading-relaxed'>
          {command}
        </pre>
        <p className='text-xs text-muted-foreground'>
          Runs anywhere <code className='font-mono'>helm</code> does — nothing but this command is
          needed. The secret is on the command line, so it lands in your shell history; the
          chart's <code className='font-mono'>controlPlane.existingSecret</code> takes one you
          manage instead.
        </p>
      </div>

      <div className='flex flex-wrap items-center justify-between gap-2 border-t pt-4'>
        <CopyButton text={clientSecret} label='Copy the secret alone' />
        <Button onClick={onDone}>Done</Button>
      </div>
    </div>
  )
}
