import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Activity, BarChart3, Server, Settings } from 'lucide-react'
import type { Deployment } from '../../types/deployment'
import { DeploymentConfigurationTab } from './components/detail/deployment-configuration-tab'
import { DeploymentDetailHeader } from './components/detail/deployment-detail-header'
import { DeploymentInsightsTab } from './components/detail/deployment-insights-tab'
import { DeploymentObservabilityTab } from './components/detail/deployment-observability-tab'
import { DeploymentOverviewTab } from './components/detail/deployment-overview-tab'
import { DeploymentStatusCards } from './components/detail/deployment-status-cards'

interface Props {
  deployment: Deployment
  onRefresh: () => void
  onBack: () => void
}

export const PageDeploymentDetail = ({ deployment, onRefresh, onBack }: Props) => {
  return (
    <div className='space-y-6'>
      <DeploymentDetailHeader 
        deployment={deployment} 
        onBack={onBack} 
        onRefresh={onRefresh} 
      />

      <DeploymentStatusCards deployment={deployment} />

      {/* Tabs Section */}
      <Tabs defaultValue='overview' className='space-y-4'>
        <TabsList>
          <TabsTrigger value='overview' className='gap-2'>
            <Server className='h-4 w-4' />
            Overview
          </TabsTrigger>
          <TabsTrigger value='observability' className='gap-2'>
            <Activity className='h-4 w-4' />
            Observability
          </TabsTrigger>
          <TabsTrigger value='insights' className='gap-2'>
            <BarChart3 className='h-4 w-4' />
            Insights
          </TabsTrigger>
          <TabsTrigger value='configuration' className='gap-2'>
            <Settings className='h-4 w-4' />
            Configuration
          </TabsTrigger>
        </TabsList>

        <TabsContent value='overview' className='space-y-6'>
          <DeploymentOverviewTab deployment={deployment} />
        </TabsContent>

        <TabsContent value='observability' className='space-y-6'>
          <DeploymentObservabilityTab />
        </TabsContent>

        <TabsContent value='insights' className='space-y-6'>
          <DeploymentInsightsTab />
        </TabsContent>

        <TabsContent value='configuration' className='space-y-6'>
          <DeploymentConfigurationTab deployment={deployment} />
        </TabsContent>
      </Tabs>
    </div>
  )
}