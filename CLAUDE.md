# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Aether is a monorepo project containing:
- **Rust workspace** (`libs/`) - Backend libraries (aether-core, aether-api)
- **React frontend** (`apps/console/`) - Web console using Vite, React 19, TypeScript, TanStack Router/Query, and Tailwind CSS

This is a polyglot codebase requiring both Rust and Node.js toolchains.

## Build & Development Commands

### Rust Libraries (workspace root)
```bash
# Build all Rust libraries
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Test a specific package
cargo test -p aether-core

# Check without building
cargo check
```

### Frontend Console (apps/console/)
```bash
cd apps/console

# Install dependencies (uses pnpm)
pnpm install

# Start development server
pnpm dev

# Build for production
pnpm build

# Run linter
pnpm lint

# Preview production build
pnpm preview
```

Note: The project uses `rolldown-vite` (specified in package.json as vite override) instead of standard Vite.

## Architecture

### Rust Workspace Structure
- `libs/aether-core/` - Core library functionality
- `libs/aether-api/` - API library functionality
- Workspace configuration in root `Cargo.toml` with shared package metadata (version, authors, edition)

### Frontend Architecture (apps/console/)

The frontend follows a **domain-driven design** pattern:

```
src/
├── domain/              # Business domains
│   ├── instances/       # Instances domain
│   │   ├── services/    # API calls and data fetching
│   │   ├── hooks/       # React Query hooks
│   │   ├── types/       # TypeScript interfaces
│   │   └── pages/
│   │       ├── feature/ # Smart components with data fetching
│   │       └── ui/      # Presentational components
│   └── dashboard/       # Dashboard domain
│       └── pages/ui/
├── components/
│   ├── ui/              # shadcn/ui components
│   ├── layout/          # Layout components (MainLayout)
│   └── [shared components] # Navigation, sidebar, etc.
├── hooks/               # Shared hooks
├── lib/                 # Utilities (utils.ts)
├── router.tsx           # TanStack Router configuration
└── main.tsx             # App entry point
```

**Key architectural patterns:**
- **Domain folders** organize features by business concern (instances, dashboard)
- **Feature vs UI pages**: Feature components handle data fetching and business logic, UI components are presentational
- **Services layer**: Data fetching functions in `services/` (e.g., `fetchInstances`)
- **Hooks layer**: React Query hooks in `hooks/` abstract data fetching (e.g., `use-instances.ts`)
- **Routing**: File-based routing using TanStack Router with type-safe routes
- **State management**: TanStack Query for server state, React hooks for local state
- **Styling**: Tailwind CSS with shadcn/ui component library (configured in `components.json`)

### Tech Stack Details

**Frontend:**
- React 19 with TypeScript
- TanStack Router for routing (with Router DevTools)
- TanStack Query for data fetching/caching
- Tailwind CSS v4 + shadcn/ui components
- Radix UI primitives
- Vite (rolldown-vite) for bundling
- SWC for Fast Refresh

**UI Components:**
- All UI components are in `components/ui/` (generated/managed by shadcn)
- Uses class-variance-authority for component variants
- Lucide React for icons

## Development Workflow

### Adding a New Domain
1. Create domain folder: `src/domain/[domain-name]/`
2. Add subdirectories: `services/`, `hooks/`, `pages/feature/`, `pages/ui/`, `types/`
3. Implement service functions for API calls
4. Create React Query hooks wrapping services
5. Build feature components (smart) and UI components (presentational)
6. Register routes in `router.tsx`

### Working with shadcn/ui
- Components are installed individually and committed to the repository
- Modify components directly in `components/ui/` as needed
- Configuration in `components.json`

### API Integration
- API endpoints are relative URLs (e.g., `/api/v1/instances`)
- Services use native `fetch` API
- React Query manages caching, refetching, and state

## TypeScript Configuration
- `tsconfig.json` - Base configuration
- `tsconfig.app.json` - Application code configuration
- `tsconfig.node.json` - Vite config and tooling

Path aliases configured: `@/` maps to `src/`

## Testing
Currently no test framework configured for the frontend. For Rust, use standard `cargo test`.
