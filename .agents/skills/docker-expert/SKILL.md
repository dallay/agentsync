---
name: docker-expert
description: >-
  Advanced Docker containerization expert for multi-stage builds, image optimization,
  security hardening, and Compose orchestration. Use when the task involves `working with
  Dockerfile`, `docker-compose.yml`, `containerization`, `multi-stage builds`, or
  `optimizing Docker images`.
license: MIT
metadata:
  version: "1.0.0"
---

## When to Use

- Creating or optimizing a `Dockerfile` for any project.
- Setting up or reviewing `docker-compose.yml` for local development or production.
- Troubleshooting container security, large image sizes, or slow build times.
- Implementing multi-stage builds to separate dependencies from runtime.

## Critical Patterns

- **Security First (Non-Root):** NEVER run containers as `root`. Always create and switch to a
  non-root user via the `USER` instruction.
- **Secrets over Env Vars:** Use Docker Secrets (`secrets:`) for sensitive data in
  `docker-compose.yml` rather than plain environment variables.
- **Multi-Stage Builds:** Always separate the builder (compilation, dependency fetching) from the
  final runtime image. Do not ship build tools to production.
- **Minimal Attack Surface:** Prefer `alpine`, `slim`, or `distroless` base images.
- **Supply Chain Integrity:** Pin base image versions down to the digest (e.g.,
  `alpine:3.21@sha256:xyz...`).
- **Layer Caching:** Order instructions by frequency of change. Copy and install dependencies
  *before* copying the rest of the source code.
- **One Layer Operations:** Chain `apt-get update` with `apt-get install` using `&&` and clear the
  cache (`rm -rf /var/lib/apt/lists/*`) in the exact same `RUN` command.

## Code Examples

### Optimized Multi-Stage Dockerfile

```dockerfile
# Stage 1: Build & Dependencies
FROM node:18-alpine AS deps
WORKDIR /app
COPY package*.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --only=production

# Stage 2: Build source
FROM node:18-alpine AS build
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build && npm prune --production

# Stage 3: Runtime
FROM node:18-alpine AS runtime
# Create non-root user explicitly
RUN addgroup -g 1001 -S nodejs && adduser -S appuser -u 1001 -G nodejs
WORKDIR /app

# Copy only what is strictly necessary
COPY --from=deps --chown=appuser:nodejs /app/node_modules ./node_modules
COPY --from=build --chown=appuser:nodejs /app/dist ./dist

# Drop privileges
USER appuser

EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:3000/health || exit 1

CMD ["node", "dist/index.js"]
```

### Production-Ready Compose Pattern

```yaml
version: '3.8'
services:
  app:
    build:
      context: .
      target: runtime
    depends_on:
      db:
        condition: service_healthy
    networks:
      - frontend
      - backend
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 512M

  db:
    image: postgres:15-alpine
    environment:
      POSTGRES_DB_FILE: /run/secrets/db_name
      POSTGRES_USER_FILE: /run/secrets/db_user
      POSTGRES_PASSWORD_FILE: /run/secrets/db_password
    secrets:
      - db_name
      - db_user
      - db_password
    networks:
      - backend
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER}"]
      interval: 10s
      timeout: 5s
      retries: 5

networks:
  frontend:
  backend:
    internal: true

secrets:
  db_name:
    external: true
  db_user:
    external: true
  db_password:
    external: true
```

## Commands

```bash
# Clean builds with no cache
docker build --pull --no-cache -t my-app:latest .

# Validate Compose configuration
docker-compose config

# Check local image sizes and tags
docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"
```

## Resources

- **Templates**: See [assets/.dockerignore](assets/.dockerignore) for a production-ready
  `.dockerignore` template to optimize build context.
