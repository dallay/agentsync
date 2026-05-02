# Troubleshooting Common GitHub Actions Issues

## 1. Workflow Not Triggering / Jobs Skipping

**Symptoms:** Workflow doesn't run on push/PR, or jobs skip unexpectedly.

**Diagnose:**

- Verify `on` triggers match the event (push vs pull_request, correct branches/paths)
- Check `paths-ignore` and `branches-ignore` — they take precedence
- For `workflow_dispatch`: workflow file must be on default branch
- Inspect `if` conditions — add debug step with `always()`:

  ```yaml
  - name: Debug context
    if: always()
    run: |
      echo "github.ref: ${{ github.ref }}"
      echo "github.event_name: ${{ github.event_name }}"
      echo '${{ toJson(github) }}'
  ```

- Check `concurrency` — a previous run may be blocking the new one
- Review branch protection rules that may prevent workflow execution

## 2. Permission Errors

**Symptoms:** `Resource not accessible by integration`, `Permission denied`, 403 errors.

**Diagnose:**

- Review `permissions` at workflow and job level
- Default `GITHUB_TOKEN` may be too restrictive if you set `permissions` globally
- Common fixes:

  ```yaml
  # Need to comment on PRs?
  permissions:
    pull-requests: write

  # Need to push packages?
  permissions:
    packages: write

  # Need OIDC tokens?
  permissions:
    id-token: write
  ```

- For environment secrets: check if manual approval is pending
- For OIDC: verify trust policy in cloud provider matches GitHub's OIDC issuer
- Secret name must match exactly: `secrets.MY_API_KEY` (case-sensitive)

## 3. Cache Issues

**Symptoms:** `Cache not found`, always misses, `Cache creation failed`.

**Diagnose:**

- Verify cache key uses `hashFiles()` on the right lockfile:

  ```yaml
  # WRONG: key never changes
  key: node-modules-cache

  # RIGHT: key changes when dependencies change
  key: ${{ runner.os }}-node-${{ hashFiles('**/package-lock.json') }}
  ```

- Verify `path` matches where dependencies are actually installed
- Use `actions/cache/restore` with `lookup-only: true` to debug key matching
- Check GitHub's cache size limits per repository (~10 GB)
- Caches are scoped to branch — PRs can read base branch caches but write to their own
- Cache entries unused for 7+ days are evicted

## 4. Slow Workflows / Timeouts

**Symptoms:** Workflows take too long, hit timeout limits, burn runner minutes.

**Fix priority:**

1. **Add caching** — biggest single improvement for most workflows
2. **Shallow clone** — `fetch-depth: 1` saves seconds to minutes on large repos
3. **Parallelize** — use `strategy.matrix` to split test suites
4. **Combine commands** — fewer steps = less overhead
5. **Set `timeout-minutes`** — prevent hung jobs from burning minutes:

   ```yaml
   jobs:
     test:
       timeout-minutes: 15
   ```

6. **Path filters** — skip workflows when irrelevant files change:

   ```yaml
   on:
     push:
       paths:
         - 'src/**'
         - 'package.json'
         - '.github/workflows/ci.yml'
   ```

## 5. Flaky Tests

**Symptoms:** Tests pass locally but fail randomly in CI.

**Common causes and fixes:**

- **Timing issues:** Replace `sleep` with explicit waits (Playwright/Cypress `waitFor`)
- **Port conflicts:** Use dynamic ports or let the framework assign them
- **Environment drift:** Pin tool versions exactly (not `node: 18`, use `node: 18.19.0`)
- **Shared state:** Ensure test isolation — clean DB between tests
- **Network:** External API calls in tests are inherently flaky — mock them
- **Debug in CI:**

  ```yaml
  - name: Run tests
    run: npm test
  - name: Upload screenshots on failure
    if: failure()
    uses: actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f # v7.0.0
    with:
      name: test-failure-screenshots
      path: screenshots/
      retention-days: 7
  ```

## 6. Deployment Failures

**Symptoms:** Deploy step succeeds but app doesn't work.

**Diagnose:**

- Check application logs immediately after deploy (`kubectl logs`, cloud provider logs)
- Verify environment variables and secrets are correctly injected
- Confirm the deployed artifact matches what was built and tested (use same artifact via
  `download-artifact`)
- Run post-deploy health check:

  ```yaml
  - name: Verify deployment
    run: |
      for i in $(seq 1 10); do
        if curl -sf https://staging.example.com/health; then
          echo "Health check passed"
          exit 0
        fi
        echo "Attempt $i failed, retrying in 10s..."
        sleep 10
      done
      echo "Health check failed after 10 attempts"
      exit 1
  ```

- If production: **rollback first, diagnose second**

## 7. Action Version Update Failures

**Symptoms:** Dependabot PR updates action version, CI breaks.

**Fix:**

- Read the action's changelog for breaking changes
- Check if `with` inputs changed between versions
- Verify the new SHA is correct:

  ```bash
  # Verify a tag's SHA
  git ls-remote https://github.com/actions/checkout.git refs/tags/v4.3.1
  ```

- Test in a branch before merging Dependabot PRs for critical actions

## Useful Debug Commands

```bash
# List recent workflow runs with status
gh run list --limit 10

# View failed run logs
gh run view <run-id> --log-failed

# Download run logs for offline analysis
gh run view <run-id> --log > workflow.log

# Re-run only failed jobs
gh run rerun <run-id> --failed

# Watch a run in real-time
gh run watch <run-id>

# Check action version SHA
gh api repos/actions/checkout/git/ref/tags/v4.3.1 --jq '.object.sha'
```
