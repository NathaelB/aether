# Aether Core

Core domain library for Aether IAM as a Service platform.

## Features

- **Domain-Driven Design (DDD)** architecture
- **Hexagonal Architecture** (Ports & Adapters)
- Type-safe value objects with validation
- Rich business logic in domain entities
- Repository pattern for persistence abstraction

## Organisation Domain

### Quick Start

```rust
use aether_core::domain::{
    organisation::{
        commands::CreateOrganisationCommand,
        value_objects::{OrganisationName, Plan},
        service::OrganisationServiceImpl,
    },
    user::UserId,
};
use uuid::Uuid;

// 1. Create a command
let name = OrganisationName::new("Acme Corp")?;
let owner_id = UserId(Uuid::new_v4());
let command = CreateOrganisationCommand::new(name, owner_id, Plan::Starter);

// 2. Create the service (inject repository)
let service = OrganisationServiceImpl::new(repository);

// 3. Create the organisation
let organisation = service.create_organisation(command).await?;

println!("Created organisation: {} with slug: {}",
    organisation.name,
    organisation.slug
);
```

### Business Rules

#### ✅ Organisation Limits per User
- **Maximum 10 active organisations** per user
- Deleted/suspended organisations don't count
- Error: `CoreError::UserOrganisationLimitReached`

```rust
// This will fail if user already has 10 active organisations
let result = service.create_organisation(command).await;

match result {
    Err(CoreError::UserOrganisationLimitReached { max, current }) => {
        println!("User has {}/{} organisations", current, max);
    }
    Ok(org) => println!("Created: {}", org.name),
    Err(e) => eprintln!("Error: {}", e),
}
```

#### ✅ Slug Uniqueness
- Slugs are globally unique across all organisations
- Auto-generated from name if not provided
- Error: `CoreError::OrganisationSlugAlreadyExists`

```rust
// Automatic slug generation
let command = CreateOrganisationCommand::new(
    OrganisationName::new("My Company")?,
    owner_id,
    Plan::Free
);
// Slug will be: "my-company"

// Manual slug
let command = CreateOrganisationCommand::new(name, owner_id, Plan::Free)
    .with_slug(OrganisationSlug::new("custom-slug")?);
```

#### ✅ Plan Limits

Each plan has resource limits:

| Plan | Instances | Users | Storage |
|------|-----------|-------|---------|
| Free | 1 | 100 | 1 GB |
| Starter | 5 | 250 | 10 GB |
| Business | 20 | 10,000 | 50 GB |
| Enterprise | 100 | Unlimited | 100 GB |

```rust
let org = Organisation::new(name, slug, owner_id, Plan::Free);

// Check limits
org.check_instance_limit(current_instances)?;  // Max 1 for Free plan
org.check_user_limit(current_users)?;          // Max 100 for Free plan
org.check_storage_limit(current_gb)?;          // Max 1 GB for Free plan
```

### Value Objects

#### OrganisationName
- 3-100 characters
- Cannot be empty or whitespace

```rust
let name = OrganisationName::new("Acme Corp")?;  // ✅ Valid
let name = OrganisationName::new("AB")?;         // ❌ Too short
let name = OrganisationName::new("")?;           // ❌ Empty
```

#### OrganisationSlug
- 3-50 characters
- Lowercase alphanumeric + hyphens only
- Cannot start/end with hyphen

```rust
let slug = OrganisationSlug::new("acme-corp")?;     // ✅ Valid
let slug = OrganisationSlug::new("Acme-Corp")?;     // ✅ Auto-lowercased
let slug = OrganisationSlug::new("-acme")?;         // ❌ Starts with hyphen
let slug = OrganisationSlug::new("acme corp")?;     // ❌ Contains space

// Auto-generate from name
let name = OrganisationName::new("Acme Corp!")?;
let slug = OrganisationSlug::from_name(&name)?;     // "acme-corp"
```

### Service Methods

#### Create Organisation

```rust
let command = CreateOrganisationCommand::new(name, owner_id, plan)
    .with_slug(custom_slug);  // Optional

let organisation = service.create_organisation(command).await?;
```

Checks:
1. ✅ User has less than 10 active organisations
2. ✅ Slug is unique
3. ✅ Name and slug are valid

#### Update Organisation

```rust
let command = UpdateOrganisationCommand::new()
    .with_name(new_name)
    .with_slug(new_slug);

let updated = service.update_organisation(org_id, command).await?;
```

Checks:
1. ✅ Organisation exists
2. ✅ Organisation is active
3. ✅ New slug is unique (if changing)

#### Delete Organisation

```rust
service.delete_organisation(org_id).await?;
```

Soft delete:
- Sets `deleted_at` timestamp
- Changes status to `Deleted`
- Organisation remains in database

### Entity Methods

```rust
let mut org = Organisation::new(name, slug, owner_id, Plan::Free);

// Status management
org.suspend()?;     // Suspend organisation
org.activate()?;    // Re-activate
org.delete()?;      // Soft delete

// Plan management
org.upgrade_plan(Plan::Business)?;

// Custom limits (for Enterprise customers)
org.set_custom_limits(OrganisationLimits::custom(100, 1000, 500));

// Name/slug update
org.update_name(new_name, new_slug);
```

## Error Handling

All errors use the `CoreError` enum:

```rust
use aether_core::domain::CoreError;

match service.create_organisation(command).await {
    Ok(org) => println!("Created: {}", org.id),

    Err(CoreError::UserOrganisationLimitReached { max, current }) => {
        eprintln!("User has reached limit: {}/{}", current, max);
    }

    Err(CoreError::OrganisationSlugAlreadyExists { slug }) => {
        eprintln!("Slug '{}' is already taken", slug);
    }

    Err(CoreError::InvalidOrganisationName { reason }) => {
        eprintln!("Invalid name: {}", reason);
    }

    Err(e) => eprintln!("Error: {}", e),
}
```

## Repository Implementation

### PostgreSQL Repository (Recommended)

The library includes a production-ready PostgreSQL implementation:

```rust
use aether_core::infrastructure::postgres::PostgresOrganisationRepository;
use sqlx::postgres::PgPool;

// 1. Setup database connection
let database_url = std::env::var("DATABASE_URL")
    .expect("DATABASE_URL must be set");
let pool = PgPool::connect(&database_url).await?;

// 2. Run migrations
sqlx::migrate!("./migrations")
    .run(&pool)
    .await?;

// 3. Create repository
let repo = PostgresOrganisationRepository::new(pool);

// 4. Use with service
let service = OrganisationServiceImpl::new(repo);
```

**Enable the `postgres` feature in your `Cargo.toml`:**

```toml
[dependencies]
aether-core = { version = "0.1", features = ["postgres"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
```

**Database Setup:**

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Set database URL
export DATABASE_URL="postgres://username:password@localhost/aether"

# Run migrations
cd libs/aether-core
sqlx migrate run
```

See [migrations/README.md](./migrations/README.md) for detailed migration documentation.

### Custom Repository Implementation

You can also implement your own repository for other databases:

```rust
use aether_core::domain::organisation::{
    ports::OrganisationRepository,
    commands::CreateOrganisationData,
    Organisation, OrganisationId,
};

struct MyRepository {
    // database connection, etc.
}

impl OrganisationRepository for MyRepository {
    async fn create(&self, data: CreateOrganisationData) -> Result<Organisation, CoreError> {
        let id = OrganisationId::new();  // Generate UUID
        let now = Utc::now();

        // Build complete organisation
        let org = Organisation {
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

        // Insert to database...

        Ok(org)
    }

    // Implement other methods...
}
```

See [ARCHITECTURE.md](./ARCHITECTURE.md) for detailed architecture documentation.

## Testing

```bash
cargo test -p aether-core
```

All value objects and entities have comprehensive unit tests.

## License

See LICENSE file in the repository root.
