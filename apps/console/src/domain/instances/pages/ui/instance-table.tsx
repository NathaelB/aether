import { Button } from '@/components/ui/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Play,
  Square,
  ExternalLink,
  Settings,
  FileText,
  Trash2,
  MoreVertical,
  Copy,
} from 'lucide-react';
import type { Instance, InstanceType } from '../../types/instance';
import { InstanceStatusBadge } from './instance-status-badge';
import { formatDistanceToNow } from 'date-fns';

interface InstanceTableProps {
  instances: Instance[];
}

const typeConfig: Record<InstanceType, { label: string; className: string }> = {
  keycloak: { label: 'Keycloak', className: 'bg-blue-100 text-blue-700' },
  ferriskey: { label: 'Ferriskey', className: 'bg-purple-100 text-purple-700' },
  authentik: { label: 'Authentik', className: 'bg-orange-100 text-orange-700' },
};

const environmentBadgeColor: Record<string, string> = {
  production: 'bg-green-100 text-green-800',
  staging: 'bg-yellow-100 text-yellow-800',
  development: 'bg-blue-100 text-blue-800',
};

export const InstanceTable = ({ instances }: InstanceTableProps) => {
  const formatDate = (date: string) => {
    try {
      return formatDistanceToNow(new Date(date), { addSuffix: true });
    } catch {
      return 'N/A';
    }
  };

  const handleCopyEndpoint = (endpoint?: string) => {
    if (endpoint) {
      navigator.clipboard.writeText(endpoint);
    }
  };

  return (
    <div className="rounded-md border">
      <Table>
        <TableHeader>
          <TableRow className="bg-muted/50">
            <TableHead className="w-[250px]">Instance</TableHead>
            <TableHead>Type</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Environment</TableHead>
            <TableHead>Version</TableHead>
            <TableHead>Region</TableHead>
            <TableHead>Last Deployment</TableHead>
            <TableHead className="text-right">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {instances.length === 0 ? (
            <TableRow>
              <TableCell colSpan={8} className="h-32 text-center">
                <div className="flex flex-col items-center justify-center gap-2">
                  <p className="text-sm text-muted-foreground">No instances found</p>
                  <p className="text-xs text-muted-foreground">
                    Deploy your first IAM instance to get started
                  </p>
                </div>
              </TableCell>
            </TableRow>
          ) : (
            instances.map((instance) => (
              <TableRow key={instance.id} className="group">
                <TableCell>
                  <div className="flex flex-col">
                    <span className="font-medium">{instance.name}</span>
                    <span className="text-xs text-muted-foreground font-mono">
                      {instance.id}
                    </span>
                  </div>
                </TableCell>
                <TableCell>
                  <span
                    className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium ${
                      typeConfig[instance.type].className
                    }`}
                  >
                    {typeConfig[instance.type].label}
                  </span>
                </TableCell>
                <TableCell>
                  <InstanceStatusBadge status={instance.status} showIcon={false} />
                </TableCell>
                <TableCell>
                  <span
                    className={`inline-flex items-center rounded-md px-2 py-1 text-xs font-medium capitalize ${
                      environmentBadgeColor[instance.environment]
                    }`}
                  >
                    {instance.environment}
                  </span>
                </TableCell>
                <TableCell className="font-mono text-xs">{instance.version}</TableCell>
                <TableCell className="text-sm text-muted-foreground">
                  {instance.region}
                </TableCell>
                <TableCell className="text-sm text-muted-foreground">
                  {formatDate(instance.lastDeployment)}
                </TableCell>
                <TableCell>
                  <div className="flex items-center justify-end gap-1">
                    {instance.status === 'running' && instance.endpoint && (
                      <>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-8 w-8 opacity-0 group-hover:opacity-100 transition-opacity"
                          onClick={() => window.open(instance.endpoint, '_blank')}
                        >
                          <ExternalLink className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-8 w-8 opacity-0 group-hover:opacity-100 transition-opacity"
                        >
                          <Square className="h-4 w-4" />
                        </Button>
                      </>
                    )}
                    {instance.status === 'stopped' && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 opacity-0 group-hover:opacity-100 transition-opacity"
                      >
                        <Play className="h-4 w-4" />
                      </Button>
                    )}
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" className="h-8 w-8">
                          <MoreVertical className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end" className="w-[200px]">
                        <DropdownMenuItem onClick={() => handleCopyEndpoint(instance.endpoint)}>
                          <Copy className="mr-2 h-4 w-4" />
                          Copy Endpoint
                        </DropdownMenuItem>
                        <DropdownMenuItem>
                          <Settings className="mr-2 h-4 w-4" />
                          Configure
                        </DropdownMenuItem>
                        <DropdownMenuItem>
                          <FileText className="mr-2 h-4 w-4" />
                          View Logs
                        </DropdownMenuItem>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem className="text-red-600 focus:text-red-600">
                          <Trash2 className="mr-2 h-4 w-4" />
                          Delete Instance
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  );
};
