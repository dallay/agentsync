---
name: github-actions
description: >-
  Comprehensive guide for building robust, secure, and efficient CI/CD pipelines using
  GitHub Actions. Use when the task involves `creating or editing GitHub Actions
  workflows`, `.github/workflows/*.yml`, `CI/CD pipelines`, `GitHub Actions best
  practices`, or `workflow optimization`.
license: MIT
metadata:
  version: "1.0.0"
---

## When to Use

- Creating or editing GitHub Actions workflow files (`.github/workflows/*.yml`)
- Reviewing CI/CD pipelines for security, performance, or correctness
- Setting up automated testing, building, or deployment pipelines
- Troubleshooting failing or flaky GitHub Actions workflows
- Designing deployment strategies (staging, production, blue/green, canary)

## Critical Patterns

- **Pin Actions to Commit SHA:** Always use full commit SHA, never mutable tags (`@v4`, `@main`).
  Add version as comment:
  `uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`. Tags can be silently
  moved to compromised commits (supply chain attack).
- **Least Privilege `GITHUB_TOKEN`:** Set `permissions: contents: read` at workflow level. Grant
  write only where explicitly needed, per-job.
- **Secrets via `secrets` Context Only:** Never hardcode sensitive data. Use `${{ secrets.NAME }}`.
  Use environment secrets for deployment targets with manual approvals.
- **OIDC Over Static Credentials:** Use OpenID Connect for cloud auth (AWS, Azure, GCP) instead of
  long-lived access keys.
- **Cache Dependencies:** Use `actions/cache` with `hashFiles()` keys for `node_modules`, pip,
  Maven, etc. to dramatically speed up builds.
- **Shallow Clone:** Use `fetch-depth: 1` in `actions/checkout` unless full history is needed.
- **Fail Fast on Security:** Integrate dependency review and SAST (CodeQL) as blocking checks.
- **Environment Protection:** Use GitHub Environments with required reviewers and branch
  restrictions for staging/production deploys.

## Workflow Structure

```yaml
name: CI/CD Pipeline

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  workflow_dispatch:
    inputs:
      environment:
        description: 'Deploy target'
        required: false
        default: 'staging'
        type: choice
        options: [ staging, production ]

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

permissions:
  contents: read  # Least privilege default

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
        with:
          fetch-depth: 1
      - uses: actions/setup-node@3235b876344d2a9aa001b8d1453c930bba69e610 # v3.9.1
        with:
          node-version: 20
          cache: 'npm'
      - run: npm ci
      - run: npm run lint

  test:
    runs-on: ubuntu-latest
    needs: lint
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - uses: actions/setup-node@3235b876344d2a9aa001b8d1453c930bba69e610 # v3.9.1
        with:
          node-version: 20
          cache: 'npm'
      - run: npm ci
      - run: npm test -- --coverage
      - uses: actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f # v7.0.0
        if: always()
        with:
          name: test-results
          path: coverage/
          retention-days: 14

  build:
    runs-on: ubuntu-latest
    needs: test
    outputs:
      artifact_name: ${{ steps.package.outputs.name }}
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - uses: actions/setup-node@3235b876344d2a9aa001b8d1453c930bba69e610 # v3.9.1
        with:
          node-version: 20
          cache: 'npm'
      - run: npm ci
      - run: npm run build
      - name: Package application
        id: package
        run: |
          zip -r dist.zip dist
          echo "name=dist.zip" >> "$GITHUB_OUTPUT"
      - uses: actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f # v7.0.0
        with:
          name: app-build
          path: dist.zip
          retention-days: 30

  deploy-staging:
    runs-on: ubuntu-latest
    needs: build
    if: github.ref == 'refs/heads/main'
    environment:
      name: staging
      url: https://staging.example.com
    steps:
      - uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8.0.1
        with:
          name: app-build
      - name: Deploy to staging
        env:
          DEPLOY_TOKEN: ${{ secrets.STAGING_DEPLOY_TOKEN }}
        run: |
          unzip dist.zip
          echo "Deploying to staging..."
          # ./deploy.sh --env staging
```

## Code Examples

### Action Pinning (Security)

```yaml
# BAD: mutable tag — vulnerable to supply chain attacks
- uses: actions/checkout@v4
- uses: some-org/some-action@main

# GOOD: immutable SHA with version comment
- uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
- uses: some-org/some-action@a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2 # v2.1.0
```

### Permissions (Least Privilege)

```yaml
# Workflow-level: restrictive default
permissions:
  contents: read

jobs:
  lint:
    # Inherits read-only — no override needed
    steps: [ ... ]

  deploy:
    permissions:
      contents: read
      deployments: write  # Only this job needs write
    steps: [ ... ]

  comment-on-pr:
    permissions:
      contents: read
      pull-requests: write  # Only this job needs PR write
    steps: [ ... ]
```

### Caching

```yaml
- name: Cache node modules
  uses: actions/cache@668228422ae6a00e4ad889ee87cd7109ec5666a7 # v5.0.4
  with:
    path: |
      ~/.npm
      node_modules
    key: ${{ runner.os }}-node-${{ hashFiles('**/package-lock.json') }}
    restore-keys: |
      ${{ runner.os }}-node-
```

### Matrix Strategy

```yaml
jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ ubuntu-latest, windows-latest ]
        node-version: [ 18, 20, 22 ]
        exclude:
          - os: windows-latest
            node-version: 18
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - uses: actions/setup-node@3235b876344d2a9aa001b8d1453c930bba69e610 # v3.9.1
        with:
          node-version: ${{ matrix.node-version }}
      - run: npm ci && npm test
```

### OIDC Cloud Authentication

```yaml
jobs:
  deploy:
    runs-on: ubuntu-latest
    permissions:
      id-token: write   # Required for OIDC
      contents: read
    steps:
      - uses: aws-actions/configure-aws-credentials@ececac1a45f3b08a01d2dd070d28d111c5fe6722 # v4.1.0
        with:
          role-to-assume: arn:aws:iam::123456789012:role/my-deploy-role
          aws-region: us-east-1
      # No static credentials stored — short-lived token via OIDC
      - run: aws s3 sync dist/ s3://my-bucket/
```

### Integration Tests with Services

```yaml
jobs:
  integration:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16-alpine
        env:
          POSTGRES_DB: testdb
          POSTGRES_USER: test
          POSTGRES_PASSWORD: test
        ports:
          - 5432:5432
        options: >-
          --health-cmd "pg_isready -U test"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - run: npm ci
      - run: npm run test:integration
        env:
          DATABASE_URL: postgresql://test:test@localhost:5432/testdb
          REDIS_URL: redis://localhost:6379
```

### Environment Protection (Production Deploy)

```yaml
jobs:
  deploy-prod:
    runs-on: ubuntu-latest
    needs: [ test, build, deploy-staging ]
    if: github.ref == 'refs/heads/main'
    environment:
      name: production
      url: https://example.com
    steps:
      - uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8.0.1
        with:
          name: app-build
      - name: Deploy to production
        env:
          PROD_API_KEY: ${{ secrets.PROD_API_KEY }}
        run: ./deploy.sh --env production
      - name: Post-deploy smoke test
        run: |
          curl -sf https://example.com/health || exit 1
          echo "Smoke test passed"
```

### Reusable Workflow

```yaml
# .github/workflows/reusable-build.yml
name: Reusable Build
on:
  workflow_call:
    inputs:
      node-version:
        required: false
        type: string
        default: '20'
    outputs:
      artifact-name:
        description: 'Name of the uploaded artifact'
        value: ${{ jobs.build.outputs.artifact-name }}

jobs:
  build:
    runs-on: ubuntu-latest
    outputs:
      artifact-name: app-build-${{ github.sha }}
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - uses: actions/setup-node@3235b876344d2a9aa001b8d1453c930bba69e610 # v3.9.1
        with:
          node-version: ${{ inputs.node-version }}
          cache: 'npm'
      - run: npm ci && npm run build
      - uses: actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f # v7.0.0
        with:
          name: app-build-${{ github.sha }}
          path: dist/

# Caller workflow
# .github/workflows/ci.yml
jobs:
  build:
    uses: ./.github/workflows/reusable-build.yml
    with:
      node-version: '20'
```

## Anti-Patterns to Avoid

| Anti-Pattern                              | Risk                                                       | Do Instead                                                            |
|-------------------------------------------|------------------------------------------------------------|-----------------------------------------------------------------------|
| `uses: actions/checkout@v4` (mutable tag) | Supply chain attack — tag can be moved to malicious commit | Pin to full SHA: `@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1` |
| No `permissions` block                    | `GITHUB_TOKEN` gets default broad access                   | Set `permissions: contents: read` at workflow level                   |
| `secrets.MY_KEY` in `echo` or logs        | Secret leakage even with masking                           | Never print secrets; use them only in env vars for commands           |
| Long-lived cloud credentials as secrets   | Large blast radius if compromised                          | Use OIDC for short-lived tokens                                       |
| `fetch-depth: 0` by default               | Slow checkout, wastes bandwidth                            | `fetch-depth: 1` unless full history needed                           |
| No `concurrency` on deploy workflows      | Race conditions, double deploys                            | Add `concurrency` with `cancel-in-progress`                           |
| Hardcoded versions in multiple places     | Drift, maintenance burden                                  | Use reusable workflows or composite actions                           |
| No `timeout-minutes` on jobs              | Hung workflows burn runner minutes                         | Set reasonable `timeout-minutes` per job                              |
| No `retention-days` on artifacts          | Storage bloat, cost increase                               | Set `retention-days` based on need (7-30 days)                        |
| Running E2E tests on every push           | Slow CI, wasted resources                                  | Run on PR and main only, or use path filters                          |

## Deployment Strategies Reference

| Strategy        | How It Works                       | Best For                       | Rollback                    |
|-----------------|------------------------------------|--------------------------------|-----------------------------|
| **Rolling**     | Gradually replaces old instances   | Stateless apps, most cases     | Redeploy previous version   |
| **Blue/Green**  | Full parallel env, switch traffic  | Zero-downtime critical apps    | Switch traffic back to blue |
| **Canary**      | Route small % to new version       | Risk-sensitive changes         | Route 100% back to stable   |
| **Dark Launch** | Deploy hidden behind feature flags | Decoupling deploy from release | Toggle flag off             |

## Commands

```bash
# Validate workflow syntax locally
actionlint .github/workflows/*.yml

# Run workflow locally with act
act push --job build

# List workflow runs
gh run list --workflow=ci.yml

# View specific run logs
gh run view <run-id> --log

# Re-run failed jobs
gh run rerun <run-id> --failed

# Trigger manual workflow
gh workflow run deploy.yml -f environment=staging

# List secrets (names only)
gh secret list
```

## Resources

- **Review Checklist**: See [references/review-checklist.md](references/review-checklist.md) for a
  comprehensive workflow review checklist
- **Troubleshooting**: See [references/troubleshooting.md](references/troubleshooting.md) for
  diagnosing common failures
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Security Hardening Guide](https://docs.github.com/en/actions/security-guides/security-hardening-for-github-actions)
- [OIDC with Cloud Providers](https://docs.github.com/en/actions/deployment/security-hardening-your-deployments)
- Related skill: [docker-expert](../docker-expert/SKILL.md) — for Dockerfile and container best
  practices
