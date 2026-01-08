import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Label } from '@/components/ui/label'
import { Shield } from 'lucide-react'
import { DeploymentType } from '../../../types/deployment'

interface Props {
  selectedType: DeploymentType;
  onSelect: (type: DeploymentType) => void;
}

export function DeploymentIdentityProviderSelector({ selectedType, onSelect }: Props) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className='text-lg flex items-center gap-2'>
          <Shield className='h-5 w-5 text-muted-foreground' />
          Identity Provider
        </CardTitle>
        <CardDescription>Select the IAM solution you want to deploy.</CardDescription>
      </CardHeader>
      <CardContent className='space-y-4'>
        <div className='grid gap-2'>
          <Label htmlFor='type'>Software Image</Label>
          <div className='grid grid-cols-3 gap-4'>
            {[
              { id: 'keycloak', name: 'Keycloak', desc: 'Open Source Identity and Access Management' },
              { id: 'ferriskey', name: 'FerrisKey', desc: 'Rust-based lightweight IAM' },
              { id: 'authentik', name: 'Authentik', desc: 'Versatile authentication provider' }
            ].map((provider) => (
              <div 
                key={provider.id}
                className={`cursor-pointer rounded-lg border p-4 hover:bg-accent transition-colors ${selectedType === provider.id ? 'border-primary ring-1 ring-primary bg-accent/50' : ''}`}
                onClick={() => onSelect(provider.id as DeploymentType)}
              >
                <div className='font-semibold'>{provider.name}</div>
                <div className='text-xs text-muted-foreground mt-1'>{provider.desc}</div>
              </div>
            ))}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
