export type DeploymentStatus =
  | 'running'
  | 'stopped'
  | 'deploying'
  | 'error'
  | 'maintenance'
  | 'deleting';
export type DeploymentType = 'keycloak' | 'ferriskey' | 'authentik';
export type Environment = 'production' | 'staging' | 'development';
export type DeploymentPlan = 'freemium' | 'starter' | 'essential' | 'premium' | 'max';

export interface Deployment {
  id: string;
  name: string;
  type: DeploymentType;
  status: DeploymentStatus;
  version: string;
  environment: Environment;
  region: string;
  endpoint?: string;
  lastDeployment: string;
  createdAt: string;
  plan: DeploymentPlan;
  capacity: number; // Number of users
  uptime?: string;
}

export interface Project {
  id: string;
  name: string;
  organization: string;
}

export interface DeploymentFilters {
  search: string;
  status?: DeploymentStatus;
  environment?: Environment;
  type?: DeploymentType;
}

export const DEPLOYMENT_CAPACITIES = [100, 250, 500, 1000, 2500, 5000, 10000]

export const DEPLOYMENT_PLANS: Record<DeploymentPlan, { 
    label: string; 
    cpu: string; 
    memory: string; 
    description: string;
    maxRealms: number;
    basePrice: number;
}> = {
  'freemium': { 
      label: 'Freemium', 
      cpu: '0.5 vCPU', 
      memory: '0.5 GiB', 
      description: 'For hobby projects',
      maxRealms: 1,
      basePrice: 0
  },
  'starter': { 
      label: 'Starter', 
      cpu: '1 vCPU', 
      memory: '2 GiB', 
      description: 'Entry-level for small teams',
      maxRealms: 100,
      basePrice: 20
  },
  'essential': { 
      label: 'Essential', 
      cpu: '2 vCPU', 
      memory: '4 GiB', 
      description: 'For growing businesses',
      maxRealms: 100,
      basePrice: 50
  },
  'premium': { 
      label: 'Premium', 
      cpu: '4 vCPU', 
      memory: '8 GiB', 
      description: 'High performance for scale',
      maxRealms: 100,
      basePrice: 100
  },
  'max': { 
      label: 'Max', 
      cpu: '8 vCPU', 
      memory: '16 GiB', 
      description: 'Mission critical workloads',
      maxRealms: 100,
      basePrice: 200
  },
}
