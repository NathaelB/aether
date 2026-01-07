import { ChevronDown, FolderOpen, Layers } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { Environment } from '@/domain/instances/types/instance';

interface ProjectEnvironmentSelectorProps {
  currentProject: string;
  currentEnvironment: Environment;
  onProjectChange?: (project: string) => void;
  onEnvironmentChange?: (environment: Environment) => void;
}

const environmentColors: Record<Environment, string> = {
  production: 'bg-green-500',
  staging: 'bg-yellow-500',
  development: 'bg-blue-500',
};

const environmentLabels: Record<Environment, string> = {
  production: 'Production',
  staging: 'Staging',
  development: 'Development',
};

export const ProjectEnvironmentSelector = ({
  currentProject,
  currentEnvironment,
  onEnvironmentChange,
}: ProjectEnvironmentSelectorProps) => {
  return (
    <div className="flex items-center gap-2">
      {/* Project Selector */}
      <div className="flex items-center gap-2 rounded-lg border bg-card px-3 py-2">
        <FolderOpen className="h-4 w-4 text-muted-foreground" />
        <span className="text-sm font-medium">{currentProject}</span>
      </div>

      {/* Separator */}
      <div className="h-6 w-px bg-border" />

      {/* Environment Selector */}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" className="gap-2">
            <Layers className="h-4 w-4" />
            <span className={`h-2 w-2 rounded-full ${environmentColors[currentEnvironment]}`} />
            <span className="text-sm font-medium">
              {environmentLabels[currentEnvironment]}
            </span>
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-[200px]">
          <DropdownMenuLabel>Select Environment</DropdownMenuLabel>
          <DropdownMenuSeparator />
          {(Object.keys(environmentLabels) as Environment[]).map((env) => (
            <DropdownMenuItem
              key={env}
              onClick={() => onEnvironmentChange?.(env)}
              className="gap-2"
            >
              <span className={`h-2 w-2 rounded-full ${environmentColors[env]}`} />
              {environmentLabels[env]}
              {env === currentEnvironment && (
                <span className="ml-auto text-xs text-muted-foreground">✓</span>
              )}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
};
