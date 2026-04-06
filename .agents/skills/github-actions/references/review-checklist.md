# GitHub Actions Workflow Review Checklist

Use this checklist when reviewing `.github/workflows/*.yml` files.

## General Structure

- [ ] Workflow has a clear, descriptive `name`
- [ ] `on` triggers are appropriate (not overly broad)
- [ ] Path/branch filters used to avoid unnecessary runs
- [ ] `concurrency` set for critical workflows or deploy pipelines
- [ ] `permissions` set at workflow level to `contents: read` (least privilege)
- [ ] Reusable workflows (`workflow_call`) used for shared patterns

## Jobs and Steps

- [ ] Jobs have clear names representing distinct phases (lint, test, build, deploy)
- [ ] `needs` dependencies correctly defined between jobs
- [ ] `outputs` used for inter-job data passing (not re-building)
- [ ] `if` conditions used for conditional execution (branch, event type, status)
- [ ] `timeout-minutes` set on long-running jobs
- [ ] All `uses` actions **pinned to full commit SHA** with version comment
- [ ] No mutable tag references (`@v4`, `@main`, `@latest`)
- [ ] `run` commands are clean, combined with `&&`, temp files cleaned up
- [ ] Environment variables at appropriate scope (workflow/job/step)

## Security

- [ ] All secrets accessed via `${{ secrets.NAME }}` — never hardcoded
- [ ] No secrets printed to logs (even if masked)
- [ ] OIDC used for cloud auth where possible (no long-lived credentials)
- [ ] `GITHUB_TOKEN` permissions explicitly scoped per-job where needed
- [ ] Dependency review / SCA integrated (e.g., `dependency-review-action`)
- [ ] SAST integrated (e.g., CodeQL) with critical findings blocking builds
- [ ] Secret scanning enabled for the repository
- [ ] Third-party actions audited before use (prefer `actions/` org)
- [ ] Self-hosted runners hardened with restricted network access (if used)
- [ ] Container image signing in place for deployment workflows (if applicable)

## Performance

- [ ] `actions/cache` used for package manager dependencies
- [ ] Cache keys use `hashFiles()` for optimal hit rates
- [ ] `restore-keys` provide fallbacks for partial cache matches
- [ ] `fetch-depth: 1` used in `actions/checkout` (unless full history needed)
- [ ] `strategy.matrix` used for parallel test/build runs
- [ ] Artifacts used for inter-job transfer (not re-fetching/re-building)
- [ ] `retention-days` set on artifacts to manage storage costs
- [ ] Git LFS optimized or disabled if not needed

## Testing

- [ ] Unit tests run early in pipeline on every push/PR
- [ ] Integration tests use `services` for dependencies (DB, Redis, etc.)
- [ ] E2E tests run against staging environment with flakiness mitigation
- [ ] Performance/load tests integrated for critical apps (with thresholds)
- [ ] Test reports uploaded as artifacts (JUnit XML, coverage, screenshots)
- [ ] Code coverage tracked and enforced with minimum threshold
- [ ] Test results integrated with GitHub Checks/Annotations

## Deployment

- [ ] Staging and production use GitHub `environment` with protection rules
- [ ] Manual approval required for production deploys
- [ ] Rollback strategy documented and automated where possible
- [ ] Post-deployment health checks / smoke tests implemented
- [ ] Deployment type appropriate for app criticality (rolling/blue-green/canary)
- [ ] Monitoring and alerting active during and after deployment

## Observability

- [ ] Adequate logging for debugging workflow failures
- [ ] Application metrics collected and exposed
- [ ] Alerts configured for critical workflow failures
- [ ] Artifact `retention-days` configured appropriately
