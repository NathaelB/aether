# Repository Guidelines

## Project Structure & Module Organization
- `apps/`: Product apps
  - `apps/console/`: React + Vite web console
  - `apps/android/`: Android client
  - `apps/control-plane/`, `apps/operator/`: Rust services
- `libs/`: Shared Rust crates (domain, API, persistence, auth, etc.)
- `k8s/`, `charts/`: Kubernetes manifests and Helm charts
- `scripts/`: Helper scripts (e.g., CRD generation)
- `.sqlx/`: SQLx query metadata for Rust build/tests

## Build, Test, and Development Commands
- Rust workspace
  - `make build` → `cargo build --workspace`
  - `make test` → runs Rust tests via `cargo nextest run`
  - `cargo test -p aether-crds` → CRD-specific tests
- Web console (`apps/console/`)
  - `npm run dev` → Vite dev server
  - `npm run build` → TypeScript + Vite build
  - `npm run lint` → ESLint

## Coding Style & Naming Conventions
- Rust: `cargo fmt` formatting; follow Rust module conventions in `libs/`.
- TypeScript/React: 2-space indentation, linted via ESLint and Prettier in `apps/console/`.
- Names: prefer `snake_case` for Rust modules/functions, `PascalCase` for React components, and `kebab-case` for file names in UI pages.

## Testing Guidelines
- Rust: use `cargo nextest run` (workspace-wide); unit tests live alongside modules.
- Frontend: no test runner configured; validate via `npm run lint` + manual checks.
- When changing SQLx queries, keep `.sqlx/` updated.

## Commit & Pull Request Guidelines
- Commits follow conventional prefixes (`feat:`, `fix:`, `refactor:`, `test:`); keep messages short and scoped.
- PRs should include:
  - concise description of changes
  - linked issue (if any)
  - screenshots for UI changes (console)
  - note on tests run (e.g., `make test`, `npm run lint`)

## Security & Configuration Notes
- Local config is read from `.env` and `apps/console/.env`.
- Never commit secrets; use environment variables for auth/DB settings.


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
