export type InstanceStatus = 'running' | 'stopped' | 'deploying' | 'error' | 'maintenance';
export type InstanceType = 'keycloak' | 'ferriskey' | 'authentik';
export type Environment = 'production' | 'staging' | 'development';

export interface Instance {
  id: string;
  name: string;
  type: InstanceType;
  status: InstanceStatus;
  version: string;
  environment: Environment;
  region: string;
  endpoint?: string;
  lastDeployment: string;
  createdAt: string;
  memory: string;
  cpu: string;
  uptime?: string;
}

export interface Project {
  id: string;
  name: string;
  organization: string;
}

export interface InstanceFilters {
  search: string;
  status?: InstanceStatus;
  environment?: Environment;
  type?: InstanceType;
}
