import { useParams } from '@tanstack/react-router'
import { useGetBackupSchedule, useGetBackups, useSetBackupSchedule } from '@/api/backup.api'
import { RequiresPermission } from '@/components/layout/requires-permission'
import { useMyPermissions } from '@/domain/organisations/hooks/use-my-permissions'
import { useResolvedOrganisationId } from '@/domain/organisations/hooks/use-resolved-organisation-id'
import { CAN } from '@/domain/organisations/permissions'
import { PageBackups } from '../ui/page-backups'

export default function PageBackupsFeature() {
  const { deploymentId } = useParams({ strict: false }) as { deploymentId?: string }
  const organisationId = useResolvedOrganisationId()
  const { can } = useMyPermissions()

  const backups = useGetBackups(organisationId ?? null, deploymentId ?? null)
  const schedule = useGetBackupSchedule(organisationId ?? null, deploymentId ?? null)
  const save = useSetBackupSchedule()

  return (
    <RequiresPermission permission={CAN.viewBackups} what='see backups'>
      <PageBackups
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
      />
    </RequiresPermission>
  )
}
