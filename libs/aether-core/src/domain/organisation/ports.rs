use std::future::Future;

use aether_auth::Identity;

use crate::domain::{
    CoreError,
    organisation::{
        Organisation, OrganisationId,
        commands::{CreateOrganisationCommand, CreateOrganisationData, UpdateOrganisationCommand},
        value_objects::{OrganisationSlug, OrganisationStatus},
    },
    user::UserId,
};

/// Service trait for organisation business logic
pub trait OrganisationService: Send + Sync {
    /// Creates a new organisation
    fn create_organisation(
        &self,
        command: CreateOrganisationCommand,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    /// Updates an existing organisation
    fn update_organisation(
        &self,
        id: OrganisationId,
        command: UpdateOrganisationCommand,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    /// Deletes an organisation
    fn delete_organisation(
        &self,
        id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get_organisations(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    fn get_organisations_by_member(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;
}

/// Repository trait for organisation persistence
///
/// This trait defines the contract for persisting and retrieving organisations
/// Following the hexagonal architecture pattern (ports & adapters)
///
/// The repository is responsible for:
/// - Generating IDs (UUID v4)
/// - Managing timestamps (created_at, updated_at, deleted_at)
/// - Persisting to the database
#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait OrganisationRepository: Send + Sync {
    /// Creates a new organisation
    /// The repository will generate the ID and timestamps
    fn create(
        &self,
        data: CreateOrganisationData,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    fn insert_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Finds an organisation by its ID
    fn find_by_id(
        &self,
        id: &OrganisationId,
    ) -> impl Future<Output = Result<Option<Organisation>, CoreError>> + Send;

    /// Finds an organisation by its slug
    fn find_by_slug(
        &self,
        slug: &OrganisationSlug,
    ) -> impl Future<Output = Result<Option<Organisation>, CoreError>> + Send;

    /// Finds all organisations owned by a user
    fn find_by_owner(
        &self,
        owner_id: &UserId,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    /// Finds all organisations where a user is a member
    fn find_by_member(
        &self,
        member_id: &UserId,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    /// Lists all organisations with optional filters
    fn list(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    /// Updates an existing organisation
    fn update(
        &self,
        organisation: Organisation,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    /// Deletes an organisation (soft delete)
    fn delete(&self, id: &OrganisationId) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Checks if a slug is already taken
    fn slug_exists(
        &self,
        slug: &OrganisationSlug,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;

    /// Counts total organisations
    fn count(&self) -> impl Future<Output = Result<usize, CoreError>> + Send;

    /// Counts organisations by status
    fn count_by_status(
        &self,
        status: OrganisationStatus,
    ) -> impl Future<Output = Result<usize, CoreError>> + Send;
}

#[cfg(any(test, feature = "test-mocks"))]
impl OrganisationRepository for std::sync::Arc<MockOrganisationRepository> {
    fn create(
        &self,
        data: CreateOrganisationData,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send {
        (**self).create(data)
    }

    fn insert_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send {
        (**self).insert_member(organisation_id, user_id)
    }

    fn find_by_id(
        &self,
        id: &OrganisationId,
    ) -> impl Future<Output = Result<Option<Organisation>, CoreError>> + Send {
        (**self).find_by_id(id)
    }

    fn find_by_slug(
        &self,
        slug: &OrganisationSlug,
    ) -> impl Future<Output = Result<Option<Organisation>, CoreError>> + Send {
        (**self).find_by_slug(slug)
    }

    fn find_by_owner(
        &self,
        owner_id: &UserId,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send {
        (**self).find_by_owner(owner_id)
    }

    fn find_by_member(
        &self,
        member_id: &UserId,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send {
        (**self).find_by_member(member_id)
    }

    fn list(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send {
        (**self).list(status, limit, offset)
    }

    fn update(
        &self,
        organisation: Organisation,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send {
        (**self).update(organisation)
    }

    fn delete(&self, id: &OrganisationId) -> impl Future<Output = Result<(), CoreError>> + Send {
        (**self).delete(id)
    }

    fn slug_exists(
        &self,
        slug: &OrganisationSlug,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send {
        (**self).slug_exists(slug)
    }

    fn count(&self) -> impl Future<Output = Result<usize, CoreError>> + Send {
        (**self).count()
    }

    fn count_by_status(
        &self,
        status: OrganisationStatus,
    ) -> impl Future<Output = Result<usize, CoreError>> + Send {
        (**self).count_by_status(status)
    }
}
