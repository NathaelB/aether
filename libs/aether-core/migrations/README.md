# Database Migrations

This directory contains SQL migrations for the Aether Core database schema.

## Prerequisites

Install sqlx-cli:

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

## Setup

Set your database URL:

```bash
export DATABASE_URL="postgres://username:password@localhost/aether"
```

## Running Migrations

### Apply all pending migrations

```bash
sqlx migrate run
```

### Revert the last migration

```bash
sqlx migrate revert
```

### Check migration status

```bash
sqlx migrate info
```

## Creating New Migrations

```bash
sqlx migrate add <migration_name>
```

This will create two files:
- `<timestamp>_<migration_name>.up.sql` - Applied when running migrations
- `<timestamp>_<migration_name>.down.sql` - Applied when reverting migrations

## Migrations

### 20260106000001 - Create organisations table

Creates the main `organisations` table with:
- Core fields: id, name, slug, owner_id
- Status tracking: status, deleted_at
- Plan and limits: plan, max_instances, max_users, max_storage_gb
- Timestamps: created_at, updated_at, deleted_at
- Indices for performance on common queries

### 20260106000002 - Create organisation_members table

Creates the `organisation_members` table for tracking organisation memberships with:
- Composite primary key: (organisation_id, user_id)
- Foreign key to organisations table with CASCADE delete
- Role-based access control (owner, admin, member, viewer)
- Created timestamp

## Database Schema

After running all migrations, your database will have:

```
organisations
├── id (UUID, PK)
├── name (VARCHAR(100))
├── slug (VARCHAR(50), UNIQUE)
├── owner_id (UUID)
├── status (VARCHAR(20))
├── plan (VARCHAR(20))
├── max_instances (INTEGER)
├── max_users (INTEGER, nullable)
├── max_storage_gb (INTEGER)
├── created_at (TIMESTAMPTZ)
├── updated_at (TIMESTAMPTZ)
└── deleted_at (TIMESTAMPTZ, nullable)

organisation_members
├── organisation_id (UUID, FK, PK)
├── user_id (UUID, PK)
├── role (VARCHAR(50))
└── created_at (TIMESTAMPTZ)
```

## Notes

- The `max_users` column is nullable to support unlimited users for Enterprise plans
- The `deleted_at` column implements soft deletes
- Organisations are never hard-deleted to maintain referential integrity
- The `organisation_members` table uses CASCADE delete when an organisation is deleted
