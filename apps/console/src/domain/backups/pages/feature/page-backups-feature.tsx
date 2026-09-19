import { useNavigate, useParams } from '@tanstack/react-router'
import {
  useCutover,
  useGetBackupSchedule,
  useGetBackups,
  useRestoreBackup,
  useSetBackupSchedule,
} from '@/api/backup.api'
import { useGetDeployment, useGetDeployments } from '@/api/deployment.api'
import { RequiresPermission } from '@/components/layout/requires-permission'
import { useMyPermissions } from '@/domain/organisations/hooks/use-my-permissions'
import { useOrganisationPath } from '@/domain/organisations/hooks/use-organisation-path'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { CAN } from '@/domain/organisations/permissions'
import { PageBackups } from '../ui/page-backups'

export default function PageBackupsFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const organisationPath = useOrganisationPath()
  const navigate = useNavigate()
  const { can } = useMyPermissions()

  const deployment = useGetDeployment(deploymentId ?? null)
  const deployments = useGetDeployments()
  const backups = useGetBackups(organisationId ?? null, deploymentId ?? null)
  const schedule = useGetBackupSchedule(organisationId ?? null, deploymentId ?? null)
  const save = useSetBackupSchedule()
  const restore = useRestoreBackup()
  const cutover = useCutover()

  return (
    <RequiresPermission permission={CAN.viewBackups} what='see backups'>
      <PageBackups
        deployment={deployment.data?.data}
        deployments={deployments.data?.data}
        backups={backups.data?.data}
        schedule={schedule.data?.data}
        isLoading={schedule.isLoading}
        isSaving={save.isPending}
        mayManage={can(CAN.manageBackups)}
        onSave={(request) => {
          if (!organisationId || !deploymentId || save.isPending) return

          save.mutate({
            path: { organisation_id: organisationId, deployment_id: deploymentId },
            body: request,
          })
        }}
        isRestoring={restore.isPending}
        restoreRefusal={restore.error instanceof Error ? restore.error.message : undefined}
        onRestore={(archive, name) => {
          if (!organisationId || !deploymentId || restore.isPending) return

          restore.mutate(
            {
              path: {
                organisation_id: organisationId,
                deployment_id: deploymentId,
                backup_id: archive.id,
              },
              body: { name },
            },
            {
              // The recovery it just provisioned, not this deployment's own
              // page: what the customer asked for is the new deployment, and
              // there is nothing more to check here that the archive list
              // does not already show.
              onSuccess: async (response) => {
                const { data } = await response.json()
                navigate({ to: organisationPath(`/deployments/${data.id}`) })
              },
            },
          )
        }}
        isCuttingOver={cutover.isPending}
        cutoverRefusal={cutover.error instanceof Error ? cutover.error.message : undefined}
        onCutover={(other) => {
          if (!organisationId || !deploymentId || cutover.isPending) return

          cutover.mutate({
            path: { organisation_id: organisationId, deployment_id: deploymentId },
            body: { demote: other.id },
          })
        }}
      />
    </RequiresPermission>
  )
}
