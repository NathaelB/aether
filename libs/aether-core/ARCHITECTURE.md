# Aether Core - Architecture

## Domain-Driven Design (DDD) Architecture

This library follows **Hexagonal Architecture** (Ports & Adapters) with DDD principles.

## Organisation Domain

### Structure

```
domain/organisation/
├── mod.rs              # Organisation entity (Aggregate Root)
├── value_objects.rs    # Value Objects (OrganisationName, OrganisationSlug, Plan, etc.)
├── commands.rs         # Commands (CreateOrganisationCommand, UpdateOrganisationCommand)
├── ports.rs            # Interfaces (OrganisationService, OrganisationRepository)
└── service.rs          # Service implementation (OrganisationServiceImpl)
```

### Key Concepts

#### 1. **Commands vs Entity**

**❌ Bad Practice:**
```rust
// Repository creates entity with temporary values
let org = Organisation::new(...);  // Creates ID and timestamps
repo.create(org);  // Repository overwrites ID and timestamps
```

**✅ Good Practice:**
```rust
// Use Commands for input
let command = CreateOrganisationCommand::new(name, owner_id, plan);
service.create_organisation(command);  // Service validates & converts

// Service converts to Data
let data = CreateOrganisationData::from_command(command)?;
repo.create(data);  // Repository generates ID and timestamps
```

#### 2. **Separation of Concerns**

| Component | Responsibility |
|-----------|----------------|
| **Command** | Input validation, business rules |
| **Service** | Orchestration, business logic, slug uniqueness |
| **Data** | Data transfer to repository |
| **Repository** | ID generation, timestamp management, persistence |
| **Entity** | Invariants, state transitions |

#### 3. **CreateOrganisationCommand**

Input from API/Controllers:
```rust
pub struct CreateOrganisationCommand {
    pub name: OrganisationName,           // Required
    pub slug: Option<OrganisationSlug>,   // Optional (auto-generated if not provided)
    pub owner_id: UserId,                 // Required
    pub plan: Plan,                       // Required
}
```

Features:
- Auto-generates slug from name if not provided
- Validates business rules
- Immutable after creation

#### 4. **CreateOrganisationData**

Data transfer to repository:
```rust
pub struct CreateOrganisationData {
    pub name: OrganisationName,
    pub slug: OrganisationSlug,        // Always present (generated or provided)
    pub owner_id: UserId,
    pub plan: Plan,
    pub limits: OrganisationLimits,    // Calculated from plan
}
```

Repository responsibilities:
- Generate `OrganisationId` (UUID v4)
- Set `created_at`, `updated_at` timestamps
- Set initial `status` (Active)
- Set `deleted_at` to None
- Persist to database
- Return complete `Organisation` entity

#### 5. **Service Flow**

```rust
async fn create_organisation(command: CreateOrganisationCommand) -> Result<Organisation> {
    // 1. Business rule: Check user organisation limit (max 10 per user)
    let user_orgs = repo.find_by_owner(&command.owner_id).await?;
    let active_count = user_orgs.iter().filter(|o| o.is_active()).count();
    if active_count >= 10 {
        return Err(CoreError::UserOrganisationLimitReached);
    }

    // 2. Generate slug if not provided
    let slug = command.get_or_generate_slug()?;

    // 3. Business rule: Check slug uniqueness
    if repo.slug_exists(&slug).await? {
        return Err(CoreError::OrganisationSlugAlreadyExists);
    }

    // 4. Convert to data
    let data = CreateOrganisationData::from_command(command)?;

    // 5. Delegate to repository (generates ID, timestamps)
    let organisation = repo.create(data).await?;

    Ok(organisation)
}
```

### Business Rules

#### Organisation Creation

1. **User Limit**: A user can own maximum **10 active organisations**
   - Deleted/suspended organisations don't count towards the limit
   - Returns `CoreError::UserOrganisationLimitReached` if exceeded

2. **Slug Uniqueness**: Organisation slugs must be globally unique
   - Auto-generated from name if not provided
   - Returns `CoreError::OrganisationSlugAlreadyExists` if taken

#### Organisation Update

1. **Active Only**: Only active organisations can be updated
   - Returns `CoreError::OrganisationSuspended` for suspended/deleted orgs

2. **Slug Change**: When changing slug, check uniqueness
   - Allows keeping same slug when updating name only

#### Organisation Deletion

1. **Soft Delete**: Organisations are never hard-deleted
   - Sets `deleted_at` timestamp
   - Changes status to `Deleted`
   - Prevents double deletion

### Example Usage

#### Creating an Organisation

```rust
use aether_core::domain::organisation::{
    commands::CreateOrganisationCommand,
    value_objects::{OrganisationName, Plan},
    service::OrganisationServiceImpl,
};

// 1. Create command (from API input)
let name = OrganisationName::new("Acme Corp")?;
let command = CreateOrganisationCommand::new(name, owner_id, Plan::Starter);

// 2. Call service
let service = OrganisationServiceImpl::new(repo);
let organisation = service.create_organisation(command).await?;

// Organisation is now persisted with:
// - Generated ID
// - Auto-generated slug: "acme-corp"
// - Created/updated timestamps
// - Active status
// - Starter plan limits (5 instances, 25 users, 10GB)
```

#### Updating an Organisation

```rust
use aether_core::domain::organisation::commands::UpdateOrganisationCommand;

let command = UpdateOrganisationCommand::new()
    .with_name(OrganisationName::new("New Name")?)
    .with_slug(OrganisationSlug::new("new-slug")?);

let updated = service.update_organisation(org_id, command).await?;
```

### Repository Implementation Guidelines

When implementing `OrganisationRepository`:

```rust
impl OrganisationRepository for PostgresOrganisationRepository {
    async fn create(&self, data: CreateOrganisationData) -> Result<Organisation> {
        let id = OrganisationId::new();  // Generate UUID
        let now = Utc::now();

        let organisation = Organisation {
            id,
            name: data.name,
            slug: data.slug,
            owner_id: data.owner_id,
            status: OrganisationStatus::Active,
            plan: data.plan,
            limits: data.limits,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        // Insert into database
        sqlx::query!(...)
            .execute(&self.pool)
            .await?;

        Ok(organisation)
    }
}
```

### Benefits of This Architecture

1. **Clear Responsibilities**
   - Commands: Input validation
   - Service: Business logic
   - Repository: Persistence
   - Entity: Invariants

2. **Testability**
   - Easy to mock repositories
   - Service logic testable independently
   - Commands are pure data structures

3. **Flexibility**
   - Repository can be PostgreSQL, MySQL, MongoDB, etc.
   - Service logic remains unchanged
   - Easy to add caching, events, etc.

4. **Type Safety**
   - Commands enforce required fields
   - Value Objects prevent invalid data
   - Repository contract is explicit

5. **Domain Purity**
   - No infrastructure concerns in domain
   - No database annotations in entities
   - Clean separation of layers
