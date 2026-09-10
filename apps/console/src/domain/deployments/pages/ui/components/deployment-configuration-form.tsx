import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Server } from 'lucide-react'
import { Environment } from '../../../types/deployment'

interface Props {
  name: string;
  setName: (name: string) => void;
  environment: Environment;
  setEnvironment: (env: Environment) => void;
  region: string;
  setRegion: (region: string) => void;
  /** Regions an existing data plane serves. See `servedRegions`. */
  regions: string[];
  regionsLoading: boolean;
}

export function DeploymentConfigurationForm({
  name,
  setName,
  environment,
  setEnvironment,
  region,
  setRegion,
  regions,
  regionsLoading,
}: Props) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className='text-lg flex items-center gap-2'>
          <Server className='h-5 w-5 text-muted-foreground' />
          Deployment Details
        </CardTitle>
        <CardDescription>Define the name and environment for your deployment.</CardDescription>
      </CardHeader>
      <CardContent className='space-y-4'>
        <div className='grid gap-2'>
          <Label htmlFor='name'>Deployment Name</Label>
          <Input
            id='name'
            placeholder='e.g., prod-auth-service'
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            className='max-w-md'
          />
          <p className='text-xs text-muted-foreground'>
            Lowercase letters, numbers, and hyphens only.
          </p>
        </div>

        <div className='grid gap-2'>
          <Label htmlFor='environment'>Environment</Label>
          <Select value={environment} onValueChange={(val) => setEnvironment(val as Environment)}>
            <SelectTrigger id='environment' className='max-w-md'>
              <SelectValue placeholder='Select environment' />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='development'>Development</SelectItem>
              <SelectItem value='staging'>Staging</SelectItem>
              <SelectItem value='production'>Production</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className='grid gap-2'>
          <Label htmlFor='region'>Region</Label>
          <Select value={region} onValueChange={setRegion} disabled={regions.length === 0}>
            <SelectTrigger id='region' className='max-w-md'>
              <SelectValue
                placeholder={regionsLoading ? 'Loading regions…' : 'Select region'}
              />
            </SelectTrigger>
            <SelectContent>
              {regions.map((served) => (
                <SelectItem key={served} value={served}>
                  {served}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {/*
            An empty list is not a loading state that never resolved -- it means
            this installation has no data plane at all, and no deployment can be
            created anywhere. Saying so beats an empty dropdown the user reloads.
          */}
          {!regionsLoading && regions.length === 0 && (
            <p className='text-xs text-destructive'>
              No data plane is registered, so there is nowhere to deploy. Register one before
              creating a deployment.
            </p>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
