import type { Instance } from '../types/instance';

// Mock data for development
const mockInstances: Instance[] = [
  {
    id: 'inst_2k9f8h3j',
    name: 'production-keycloak',
    type: 'keycloak',
    status: 'running',
    version: '23.0.3',
    environment: 'production',
    region: 'us-east-1',
    endpoint: 'https://auth.company.com',
    lastDeployment: '2025-01-05T14:22:00Z',
    createdAt: '2024-12-15T10:30:00Z',
    memory: '4GB',
    cpu: '2 vCPU',
    uptime: '21d 14h',
  },
  {
    id: 'inst_7x2m5n8p',
    name: 'staging-ferriskey',
    type: 'ferriskey',
    status: 'running',
    version: '0.8.2',
    environment: 'staging',
    region: 'eu-west-1',
    endpoint: 'https://auth-staging.company.com',
    lastDeployment: '2025-01-06T09:45:00Z',
    createdAt: '2024-11-20T08:15:00Z',
    memory: '2GB',
    cpu: '1 vCPU',
    uptime: '47d 6h',
  },
  {
    id: 'inst_4b6c9d1e',
    name: 'dev-authentik',
    type: 'authentik',
    status: 'stopped',
    version: '2024.1.1',
    environment: 'development',
    region: 'us-west-2',
    endpoint: 'https://auth-dev.company.com',
    lastDeployment: '2025-01-04T11:30:00Z',
    createdAt: '2025-01-02T16:45:00Z',
    memory: '1GB',
    cpu: '1 vCPU',
  },
  {
    id: 'inst_9h3k7m2q',
    name: 'testing-keycloak',
    type: 'keycloak',
    status: 'deploying',
    version: '22.0.5',
    environment: 'staging',
    region: 'ap-southeast-1',
    endpoint: 'https://auth-test.company.com',
    lastDeployment: '2025-01-06T15:10:00Z',
    createdAt: '2024-12-28T13:20:00Z',
    memory: '2GB',
    cpu: '1 vCPU',
  },
  {
    id: 'inst_5f8g2h4j',
    name: 'backup-keycloak',
    type: 'keycloak',
    status: 'error',
    version: '23.0.3',
    environment: 'production',
    region: 'us-east-1',
    endpoint: 'https://auth-backup.company.com',
    lastDeployment: '2025-01-05T18:00:00Z',
    createdAt: '2024-10-10T09:00:00Z',
    memory: '4GB',
    cpu: '2 vCPU',
  },
  {
    id: 'inst_3m8n2p5k',
    name: 'api-gateway-keycloak',
    type: 'keycloak',
    status: 'running',
    version: '23.0.3',
    environment: 'production',
    region: 'us-east-1',
    endpoint: 'https://api-auth.company.com',
    lastDeployment: '2025-01-03T08:15:00Z',
    createdAt: '2024-09-12T14:20:00Z',
    memory: '8GB',
    cpu: '4 vCPU',
    uptime: '116d 9h',
  },
  {
    id: 'inst_6j9k4l2m',
    name: 'maintenance-authentik',
    type: 'authentik',
    status: 'maintenance',
    version: '2024.1.1',
    environment: 'staging',
    region: 'eu-west-1',
    endpoint: 'https://maint-auth.company.com',
    lastDeployment: '2025-01-06T12:00:00Z',
    createdAt: '2024-11-05T10:30:00Z',
    memory: '2GB',
    cpu: '1 vCPU',
  },
];

export const fetchInstances = async (): Promise<Instance[]> => {
  // Simulate network delay
  await new Promise((resolve) => setTimeout(resolve, 800));

  // Uncomment when API is ready:
  // const response = await fetch('/api/v1/instances');
  // if (!response.ok) throw new Error('Network error');
  // return response.json();

  return mockInstances;
};
