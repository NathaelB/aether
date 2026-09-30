import { useState } from 'react'
import { format, formatDistanceToNow } from 'date-fns'
import { ArrowUpCircle } from 'lucide-react'
import type { Schemas } from '@/api/api.client'
import { EmptyState, Page, PageTitle, Section } from '@/components/layout/page'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { StatusBadge } from '@/components/ui/status-badge'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  COMPONENTS,
  toRequest,
  whatIsMissing,
  type UpgradeComponent,
  type UpgradeForm,
  type UpgradeRow,
} from '../../dataplane-upgrades'

interface Props {
  rows: UpgradeRow[]
  dataplanes: Schemas.DataPlane[]
  isLoading: boolean
  canOperate: boolean
  isStarting: boolean
  refusal?: string
  started: boolean
  onStart: (request: Schemas.UpgradeDataplanesRequest) => void
}

const EMPTY_FORM: UpgradeForm = {
  targetVersion: '',
  allDataplanes: false,
  dataplaneIds: [],
  components: [...COMPONENTS],
  maxUnavailable: '1',
}

function toggle<T>(list: T[], item: T): T[] {
  return list.includes(item) ? list.filter((entry) => entry !== item) : [...list, item]
}

function Check({
  label,
  checked,
  onChange,
}: {
  label: string
  checked: boolean
  onChange: () => void
}) {
  return (
    <label className='flex items-center gap-2 text-sm'>
      <input type='checkbox' checked={checked} onChange={onChange} />
      {label}
    </label>
  )
}

function StartForm({
  dataplanes,
  isStarting,
  refusal,
  started,
  onStart,
}: Omit<Props, 'rows' | 'isLoading' | 'canOperate'>) {
  const [form, setForm] = useState(EMPTY_FORM)
  const missing = whatIsMissing(form)
  const everyComponent = COMPONENTS.every((component) => form.components.includes(component))

  return (
    <form
      className='space-y-5 rounded-lg border bg-card p-5'
      onSubmit={(event) => {
        event.preventDefault()
        if (!missing) onStart(toRequest(form))
      }}
    >
      <div className='space-y-2'>
        <Label htmlFor='target-version'>Target version</Label>
        <Input
          id='target-version'
          className='max-w-xs font-mono'
          placeholder='1.4.0'
          value={form.targetVersion}
          onChange={(event) => setForm({ ...form, targetVersion: event.target.value })}
        />
      </div>

      <fieldset className='space-y-2'>
        <legend className='text-sm font-medium'>Data planes</legend>
        <Check
          label='All data planes'
          checked={form.allDataplanes}
          onChange={() => setForm({ ...form, allDataplanes: !form.allDataplanes })}
        />
        {!form.allDataplanes &&
          dataplanes.map((dataplane) => (
            <Check
              key={dataplane.id}
              label={`${dataplane.region} · ${dataplane.id}`}
              checked={form.dataplaneIds.includes(dataplane.id)}
              onChange={() =>
                setForm({ ...form, dataplaneIds: toggle(form.dataplaneIds, dataplane.id) })
              }
            />
          ))}
      </fieldset>

      <fieldset className='space-y-2'>
        <legend className='text-sm font-medium'>Components</legend>
        <Check
          label='All'
          checked={everyComponent}
          onChange={() =>
            setForm({ ...form, components: everyComponent ? [] : [...COMPONENTS] })
          }
        />
        {COMPONENTS.map((component: UpgradeComponent) => (
          <Check
            key={component}
            label={component}
            checked={form.components.includes(component)}
            onChange={() => setForm({ ...form, components: toggle(form.components, component) })}
          />
        ))}
      </fieldset>

      <div className='space-y-2'>
        <Label htmlFor='max-unavailable'>At most unavailable at once</Label>
        <Input
          id='max-unavailable'
          type='number'
          min={1}
          className='w-24'
          value={form.maxUnavailable}
          onChange={(event) => setForm({ ...form, maxUnavailable: event.target.value })}
        />
      </div>

      <p className='text-xs text-muted-foreground'>Upgrades roll out one after the other.</p>

      {refusal && (
        <p role='alert' className='text-sm text-destructive'>
          {refusal}
        </p>
      )}

      {started && !refusal && (
        <p className='text-sm text-muted-foreground'>Upgrade requested.</p>
      )}

      <div className='flex items-center gap-3'>
        <Button type='submit' disabled={isStarting || missing !== undefined}>
          {isStarting ? 'Starting…' : 'Start upgrade'}
        </Button>
        {missing && <span className='text-xs text-muted-foreground'>{missing}</span>}
      </div>
    </form>
  )
}

export function PageDataplaneUpgrades({ rows, isLoading, canOperate, ...form }: Props) {
  return (
    <Page>
      <PageTitle title='Data plane upgrades' />

      <p className='mt-4 max-w-2xl text-sm text-muted-foreground'>
        The status shown is that of the request, not of the upgrade itself. How far an upgrade has
        got lives in the cluster, and is not shown here.
      </p>

      {canOperate && (
        <div className='mt-6'>
          <Section title='Start an upgrade'>
            <StartForm {...form} />
          </Section>
        </div>
      )}

      <div className='mt-6'>
        {isLoading ? (
          <div className='space-y-2'>
            <Skeleton className='h-12' />
            <Skeleton className='h-12' />
          </div>
        ) : rows.length === 0 ? (
          <EmptyState
            icon={<ArrowUpCircle className='h-5 w-5' />}
            title='No upgrade has been requested'
            description='Once an upgrade is started for a data plane, its request is listed here.'
          />
        ) : (
          <div className='rounded-lg border bg-card'>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Data plane</TableHead>
                  <TableHead>Target version</TableHead>
                  <TableHead>Request status</TableHead>
                  <TableHead>Requested</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((row) => (
                  <TableRow key={row.actionId}>
                    <TableCell className='font-mono text-xs'>{row.dataplaneId}</TableCell>
                    <TableCell className='font-mono text-xs'>{row.targetVersion ?? '—'}</TableCell>
                    <TableCell>
                      <StatusBadge tone={row.status.tone}>{row.status.label}</StatusBadge>
                    </TableCell>
                    <TableCell className='text-xs text-muted-foreground'>
                      <time
                        dateTime={row.createdAt}
                        title={format(new Date(row.createdAt), 'PPpp')}
                      >
                        {formatDistanceToNow(new Date(row.createdAt), { addSuffix: true })}
                      </time>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>
    </Page>
  )
}
