<div align="center">

# ⚡ Aether

**The Modern IAM-as-a-Service Platform**

Deploy, manage, and scale identity and access management instances with ease.

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/react-%2320232a.svg?style=for-the-badge&logo=react&logoColor=%2361DAFB)](https://reactjs.org/)
[![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=for-the-badge&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![PostgreSQL](https://img.shields.io/badge/postgres-%23316192.svg?style=for-the-badge&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

[Features](#-features) • [Quick Start](#-quick-start) • [Architecture](#-architecture) • [Documentation](#-documentation) • [Contributing](#-contributing)

</div>

---

## 🎯 Overview

Aether is a production-ready platform that simplifies the deployment and management of Identity and Access Management (IAM) solutions. Whether you're running Keycloak, Ferriskey, Authentik, or other IAM providers, Aether provides a unified interface to manage multiple instances across different environments.

### Why Aether?

- **🚀 Deploy in Minutes** - Launch IAM instances with a single click
- **📊 Centralized Management** - Monitor and control all your IAM instances from one dashboard
- **🔒 Enterprise-Ready** - Built-in multi-tenancy with organization and role management
- **💾 Automatic Backups** - Never lose your identity data
- **📈 Scalable Architecture** - From startups to enterprises
- **🎨 Modern UI** - Beautiful, responsive interface built with React 19

---

## ✨ Features

### Core Platform

- **Multi-Organization Support** - Manage up to 10 organizations per user
- **Flexible Plans** - Free, Starter, Business, and Enterprise tiers
- **Instance Management** - Deploy, monitor, and control IAM instances
- **Backup & Recovery** - Automated backups with point-in-time recovery
- **Real-time Monitoring** - Track instance health and performance
- **Role-Based Access Control** - Fine-grained permissions (Owner, Admin, Member, Viewer)

### Developer Experience

- **Clean Architecture** - Domain-Driven Design with Hexagonal Architecture
- **Type Safety** - End-to-end type safety with Rust and TypeScript
- **Modern Stack** - Latest technologies and best practices
- **API-First** - RESTful API with comprehensive documentation
- **Extensible** - Plugin system for custom integrations

---

## 🏗️ Architecture

Aether follows a modern monorepo architecture with clear separation of concerns:

```
aether/
├── apps/
│   └── console/          # React 19 frontend application
│       ├── src/
│       │   ├── components/   # Reusable UI components
│       │   ├── domain/       # Domain logic and services
│       │   └── routes/       # TanStack Router pages
│       └── package.json
│
├── libs/
│   ├── aether-core/      # Core domain library (Rust)
│   │   ├── src/
│   │   │   ├── domain/       # Business logic and entities
│   │   │   └── infrastructure/ # PostgreSQL repositories
│   │   ├── migrations/    # Database migrations
│   │   └── Cargo.toml
│   │
│   └── aether-api/       # API server (Rust)
│       └── Cargo.toml
│
└── Cargo.toml            # Workspace configuration
```

### Technology Stack

#### Backend
- **Rust** - Systems programming language for performance and safety
- **sqlx** - Compile-time verified SQL queries
- **PostgreSQL** - Production-grade relational database
- **Tokio** - Asynchronous runtime
- **thiserror** - Ergonomic error handling

#### Frontend
- **React 19** - Latest React with concurrent features
- **TypeScript** - Type-safe JavaScript
- **TanStack Router** - Type-safe routing
- **TanStack Query** - Powerful data synchronization
- **Tailwind CSS v4** - Modern utility-first CSS
- **shadcn/ui** - Beautiful, accessible components
- **Radix UI** - Unstyled, accessible primitives

### Design Principles

- **Domain-Driven Design (DDD)** - Rich domain models with business logic
- **Hexagonal Architecture** - Clean separation of domain and infrastructure
- **CQRS Pattern** - Command/Query separation for clarity
- **Repository Pattern** - Abstract data access with trait-based interfaces
- **Type-Safe Queries** - Compile-time SQL verification with sqlx

---

## 🚀 Quick Start

### Prerequisites

- **Rust** 1.75+ ([install](https://rustup.rs/))
- **Node.js** 20+ ([install](https://nodejs.org/))
- **pnpm** 8+ (`npm install -g pnpm`)
- **PostgreSQL** 15+ ([install](https://www.postgresql.org/download/))
- **sqlx-cli** (`cargo install sqlx-cli --no-default-features --features postgres`)

### Installation

1. **Clone the repository**

```bash
git clone https://github.com/yourusername/aether.git
cd aether
```

2. **Setup the database**

```bash
# Create database
createdb aether

# Set database URL
export DATABASE_URL="postgres://username:password@localhost/aether"

# Run migrations
cd libs/aether-core
sqlx migrate run
cd ../..
```

3. **Start the backend**

```bash
# Build and run the API server
cargo run -p aether-api
```

4. **Start the frontend**

```bash
# Install dependencies
cd apps/console
pnpm install

# Start development server
pnpm dev
```

5. **Open your browser**

Navigate to [http://localhost:5173](http://localhost:5173)

---

## 📖 Documentation

### Core Library

- [aether-core Documentation](./libs/aether-core/README.md) - Core domain library
- [Architecture Guide](./libs/aether-core/ARCHITECTURE.md) - DDD/Hexagonal architecture
- [Migration Guide](./libs/aether-core/migrations/README.md) - Database migrations

### For Developers

- [Claude Code Guide](./CLAUDE.md) - Development workflow and conventions
- [API Documentation](#) - RESTful API reference (coming soon)
- [Frontend Guide](#) - React application structure (coming soon)

### Key Concepts

#### Organizations

Organizations are the top-level entity in Aether. Each user can own up to 10 active organizations:

```rust
// Create an organization
let command = CreateOrganisationCommand::new(
    OrganisationName::new("Acme Corp")?,
    owner_id,
    Plan::Starter
);

let org = service.create_organisation(command).await?;
```

#### Plans & Limits

| Plan | Instances | Users | Storage |
|------|-----------|-------|---------|
| Free | 1 | 100 | 1 GB |
| Starter | 5 | 250 | 10 GB |
| Business | 20 | 10,000 | 50 GB |
| Enterprise | 100 | Unlimited | 100 GB |

#### Instance Management

Deploy and manage IAM instances across multiple environments:

- **Production** - High-availability deployments
- **Staging** - Pre-production testing
- **Development** - Local development instances

---

## 🛠️ Development

### Running Tests

```bash
# Run all tests
cargo test

# Run core library tests
cargo test -p aether-core

# Run with coverage
cargo test --all-features
```

### Building for Production

```bash
# Build backend
cargo build --release

# Build frontend
cd apps/console
pnpm build
```

### Code Quality

```bash
# Format code
cargo fmt
cd apps/console && pnpm format

# Lint code
cargo clippy
cd apps/console && pnpm lint

# Type check
cd apps/console && pnpm typecheck
```

---

## 📊 Project Status

Aether is currently in **active development**. The core features are functional, but the project is not yet production-ready.

### Completed ✅

- [x] Core domain models (Organisation, User)
- [x] PostgreSQL repository implementation
- [x] Database migrations
- [x] Organisation service with business rules
- [x] Frontend dashboard and navigation
- [x] Instance management UI
- [x] Modern, responsive design

### In Progress 🚧

- [ ] API server implementation
- [ ] Authentication and authorization
- [ ] Instance deployment logic
- [ ] Backup and restore functionality
- [ ] Monitoring and alerting

### Planned 📋

- [ ] Multi-cloud support (AWS, GCP, Azure)
- [ ] Custom domain support
- [ ] API key management
- [ ] Audit logging
- [ ] Webhooks and integrations
- [ ] CLI tool

---

## 🤝 Contributing

We welcome contributions! Please see our contributing guidelines (coming soon).

### Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style

- **Rust**: Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- **TypeScript**: Use ESLint and Prettier configurations
- **Commits**: Follow [Conventional Commits](https://www.conventionalcommits.org/)

---

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

Built with:

- [Rust](https://www.rust-lang.org/) - The Rust Programming Language
- [React](https://react.dev/) - A JavaScript library for building user interfaces
- [sqlx](https://github.com/launchbadge/sqlx) - The Rust SQL Toolkit
- [TanStack](https://tanstack.com/) - High-quality open-source software for web developers
- [Tailwind CSS](https://tailwindcss.com/) - A utility-first CSS framework
- [shadcn/ui](https://ui.shadcn.com/) - Beautifully designed components

---

<div align="center">

**[⬆ back to top](#-aether)**

Made with ❤️ by [nathaelb](https://github.com/nathaelb)

</div>
