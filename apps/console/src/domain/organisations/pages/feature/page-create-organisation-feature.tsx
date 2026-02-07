import { useMemo, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { PageCreateOrganisation } from '../ui/page-create-organisation'
import { useCreateOrganisation } from '@/api/organisation.api'
import {
  selectOrganisations,
  useOrganisationsStore,
} from '@/stores/organisations'

const slugify = (value: string) =>
  value
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/(^-|-$)+/g, '')

export default function PageCreateOrganisationFeature() {
  const navigate = useNavigate()
  const organisations = useOrganisationsStore(selectOrganisations)
  const setOrganisations = useOrganisationsStore((state) => state.setOrganisations)
  const setActiveOrganisationId = useOrganisationsStore(
    (state) => state.setActiveOrganisationId
  )
  const createOrganisation = useCreateOrganisation()
  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')
  const [isSlugEdited, setIsSlugEdited] = useState(false)

  const derivedSlug = useMemo(() => slugify(name), [name])

  const handleNameChange = (value: string) => {
    setName(value)
    if (!isSlugEdited) {
      setSlug(slugify(value))
    }
  }

  const handleSlugChange = (value: string) => {
    setSlug(value)
    setIsSlugEdited(true)
  }

  const handleSubmit = () => {
    if (!name.trim()) {
      return
    }

    createOrganisation.mutate(
      { body: { name: name.trim() } },
      {
        onSuccess: (response) => {
          const created = response?.data
          if (created) {
            setOrganisations([...organisations, created])
            setActiveOrganisationId(created.id)
          }
          if (created?.id) {
            navigate({ to: `/organisations/${created.id}` })
            return
          }
          navigate({ to: '/organisations/create' })
        },
      }
    )
  }

  return (
    <PageCreateOrganisation
      name={name}
      slug={isSlugEdited ? slug : derivedSlug}
      onNameChange={handleNameChange}
      onSlugChange={handleSlugChange}
      onSubmit={handleSubmit}
      isSubmitting={createOrganisation.isPending}
      error={createOrganisation.isError ? 'Unable to create organisation. Please try again.' : null}
    />
  )
}
