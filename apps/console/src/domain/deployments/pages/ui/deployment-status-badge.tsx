import type { InstanceStatus } from '../../types/instance'
import { CheckCircle2, XCircle, Loader2, AlertTriangle, Wrench } from 'lucide-react'

interface InstanceStatusBadgeProps {
  status: InstanceStatus;
  showIcon?: boolean;
}

const statusConfig: Record<
  InstanceStatus,
  {
    label: string;
    icon: typeof CheckCircle2;
    className: string;
    dotClassName: string;
  }
> = {
  running: {
    label: 'Running',
    icon: CheckCircle2,
    className: 'bg-green-50 text-green-700 border-green-200',
    dotClassName: 'bg-green-500',
  },
  stopped: {
    label: 'Stopped',
    icon: XCircle,
    className: 'bg-gray-50 text-gray-700 border-gray-200',
    dotClassName: 'bg-gray-400',
  },
  deploying: {
    label: 'Deploying',
    icon: Loader2,
    className: 'bg-blue-50 text-blue-700 border-blue-200',
    dotClassName: 'bg-blue-500',
  },
  error: {
    label: 'Error',
    icon: AlertTriangle,
    className: 'bg-red-50 text-red-700 border-red-200',
    dotClassName: 'bg-red-500',
  },
  maintenance: {
    label: 'Maintenance',
    icon: Wrench,
    className: 'bg-yellow-50 text-yellow-700 border-yellow-200',
    dotClassName: 'bg-yellow-500',
  },
}

export const InstanceStatusBadge = ({ status, showIcon = true }: InstanceStatusBadgeProps) => {
  const config = statusConfig[status]
  const Icon = config.icon

  return (
    <div className='inline-flex items-center gap-1.5'>
      <span className={`flex h-2 w-2 rounded-full ${config.dotClassName}`} />
      <span
        className={`inline-flex items-center gap-1 rounded-md border px-2 py-1 text-xs font-medium ${config.className}`}
      >
        {showIcon && (
          <Icon
            className={`h-3 w-3 ${status === 'deploying' ? 'animate-spin' : ''}`}
          />
        )}
        {config.label}
      </span>
    </div>
  )
}
