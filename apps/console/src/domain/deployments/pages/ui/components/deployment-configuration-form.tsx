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
}

export function DeploymentConfigurationForm({ 
  name, 
  setName, 
  environment, 
  setEnvironment, 
  region, 
  setRegion 
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
          <Select value={region} onValueChange={setRegion}>
            <SelectTrigger id='region' className='max-w-md'>
              <SelectValue placeholder='Select region' />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='us-east-1'>US East (N. Virginia)</SelectItem>
              <SelectItem value='eu-west-1'>EU West (Ireland)</SelectItem>
              <SelectItem value='ap-southeast-1'>Asia Pacific (Singapore)</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </CardContent>
    </Card>
  )
}
