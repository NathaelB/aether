import type { Deployment } from '../types/deployment'

const mockDeployments: Deployment[] = [
  {
    id: 'inst-01',
    name: 'prod-keycloak',
    type: 'keycloak',
    status: 'running',
    version: '23.0.1',
    environment: 'production',
    region: 'us-east-1',
    endpoint: 'https://auth.example.com',
    lastDeployment: '2023-10-25T10:30:00Z',
    createdAt: '2023-09-15T08:00:00Z',
    plan: 'essential',
    capacity: 2500,
    uptime: '15d 4h 23m'
  },
  {
    id: 'inst-02',
    name: 'dev-authentik',
    type: 'authentik',
    status: 'stopped',
    version: '2023.10',
    environment: 'development',
    region: 'eu-west-1',
    lastDeployment: '2023-11-02T14:15:00Z',
    createdAt: '2023-10-20T09:45:00Z',
    plan: 'freemium',
    capacity: 100,
    uptime: '0m'
  },
  {
    id: 'inst-03',
    name: 'staging-ferriskey',
    type: 'ferriskey',
    status: 'deploying',
    version: '0.5.0',
    environment: 'staging',
    region: 'us-west-2',
    lastDeployment: '2023-11-05T11:20:00Z',
    createdAt: '2023-11-01T16:30:00Z',
    plan: 'starter',
    capacity: 500,
    uptime: '1m 30s'
  },
  {
    id: 'inst-04',
    name: 'test-keycloak',
    type: 'keycloak',
    status: 'error',
    version: '23.0.0',
    environment: 'development',
    region: 'ap-southeast-1',
    lastDeployment: '2023-11-04T09:10:00Z',
    createdAt: '2023-11-04T09:00:00Z',
    plan: 'freemium',
    capacity: 100,
    uptime: '0m'
  },
  {
    id: 'inst-05',
    name: 'legacy-auth',
    type: 'keycloak',
    status: 'maintenance',
    version: '21.1.2',
    environment: 'production',
    region: 'eu-central-1',
    endpoint: 'https://legacy-auth.example.com',
    lastDeployment: '2023-08-12T00:00:00Z',
    createdAt: '2023-01-10T00:00:00Z',
    plan: 'premium',
    capacity: 5000,
    uptime: '45d 12h 0m'
  }
]

export const fetchDeployments = async (): Promise<Deployment[]> => {
  // Simulate API delay
  await new Promise(resolve => setTimeout(resolve, 800))
  
  // In a real app, this would be:
  // const response = await fetch('/api/v1/deployments');
  // if (!response.ok) throw new Error('Failed to fetch deployments');
  // return response.json();
  
  return mockDeployments
}

export const getDeploymentById = async (id: string): Promise<Deployment | undefined> => {
  await new Promise(resolve => setTimeout(resolve, 500))
  return mockDeployments.find(i => i.id === id)
}
