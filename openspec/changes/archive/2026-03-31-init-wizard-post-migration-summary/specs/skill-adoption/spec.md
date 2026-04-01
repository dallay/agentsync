# Delta for Skill Adoption

## ADDED Requirements

### Requirement: Wizard Summary Identifies Canonical Source Of Truth

After `agentsync init --wizard` migrates agent assets into `.agents/`, the completion summary MUST describe `.agents/` as the canonical source of truth for the generated configuration and migrated content.

The summary MUST describe downstream agent-specific files as follow-on targets to be reconciled later, not as the authoritative source after migration.

#### Scenario: Summary declares canonical `.agents/` ownership after migration

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard migrates one or more agent files into `.agents/`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST state that `.agents/` is the canonical source of truth
- AND the summary MUST describe the migrated config or content as now living under `.agents/`

#### Scenario: Summary does not treat legacy agent paths as authoritative

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard migrates content from one or more agent-specific directories
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT describe the original agent-specific locations as the ongoing source of truth

### Requirement: Wizard Summary States Apply As The Next Required Step

When `agentsync init --wizard` completes, the final post-migration summary MUST tell the user that running `agentsync apply` is the next step required to reconcile managed downstream files.

The summary MUST NOT claim or imply that `agentsync apply` has already run.

The summary MAY explain that `agentsync apply` can update managed targets according to the generated configuration, but it MUST present those updates as future follow-up work rather than completed work.

#### Scenario: Summary instructs user to run apply next

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST tell the user to run `agentsync apply` next
- AND the summary MUST describe `apply` as the step that reconciles downstream managed files

#### Scenario: Summary avoids claiming apply already ran

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT say or imply that `agentsync apply` already ran
- AND the summary MUST NOT present downstream targets as already reconciled solely because init completed

### Requirement: Wizard Summary Gives Cautious Git-State Guidance

The final post-migration summary MUST give cautious git guidance based only on facts known during the wizard run.

The summary MUST tell the user to review repository changes with normal git workflows after running the wizard and any follow-up apply step.

The summary MUST explain that the exact file changes shown by git MAY vary by repository depending on what files, managed blocks, or prior agent artifacts already existed.

The summary MUST NOT claim that the repository is clean, dirty, staged, unstaged, ready to commit, or otherwise describe actual git state that the wizard did not inspect.

#### Scenario: Summary warns that git changes depend on repository history

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST tell the user to review git changes manually
- AND the summary MUST explain that repo-specific differences may appear depending on what existed before

#### Scenario: Summary does not overstate current git status

- GIVEN the user completes `agentsync init --wizard`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT claim that git changes are already reviewed, complete, or safe to commit
- AND the summary MUST NOT report any staged or unstaged state unless the wizard actually inspected git state

### Requirement: Wizard Summary Explains Default Gitignore-Managed Collaboration Expectations

When the generated configuration keeps gitignore management enabled, the final post-migration summary MUST explain collaborator expectations for that mode.

That explanation MUST state that collaborators who use the repo SHOULD run `agentsync apply` so managed targets, including `.gitignore` behavior governed by configuration, stay aligned with `.agents/`.

The summary MAY warn that `apply` can introduce repository-specific `.gitignore` changes in that mode, but it MUST NOT claim that `.gitignore` has already been updated during the wizard.

#### Scenario: Default gitignore-managed mode includes collaborator warning

- GIVEN the user completes `agentsync init --wizard`
- AND the generated configuration keeps `[gitignore].enabled = true`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST explain that collaborators are expected to use `agentsync apply` to stay aligned with the canonical `.agents/` config
- AND the summary MUST mention that `.gitignore` behavior is still governed by configuration during apply

#### Scenario: Default gitignore-managed mode does not overclaim `.gitignore` changes

- GIVEN the user completes `agentsync init --wizard`
- AND the generated configuration keeps `[gitignore].enabled = true`
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT say that `.gitignore` was already updated
- AND the summary MUST NOT say that `.gitignore` requires no further review

### Requirement: Wizard Summary Reports Backup Outcomes When Relevant

If the wizard creates backups of migrated source files or directories, the final post-migration summary MUST report that backup outcome.

The backup messaging MUST identify that backups were created and SHOULD tell the user where they were written or how to find them.

If no backup was created, the summary MUST NOT imply that a backup exists.

#### Scenario: Summary reports created backup

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard creates a backup of migrated source files or directories
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST say that a backup was created
- AND the summary SHOULD identify the backup location or how to find it

#### Scenario: Summary stays accurate when no backup exists

- GIVEN the user runs `agentsync init --wizard`
- AND the wizard does not create any backup
- WHEN the wizard prints its final post-migration summary
- THEN the summary MUST NOT imply that a backup was created

### Requirement: Wizard Completion Output Avoids Conflicting Generic Footer Messaging

When the wizard prints its purpose-built post-migration summary, the overall command completion output MUST remain coherent and MUST NOT include duplicate or conflicting generic footer guidance.

If a generic init footer would repeat, contradict, or oversimplify wizard-specific migration guidance, the system MUST suppress or replace that generic footer for wizard runs.

#### Scenario: Wizard run emits one coherent completion message

- GIVEN the user completes `agentsync init --wizard`
- WHEN command completion output is printed
- THEN the output MUST contain one coherent set of post-migration next steps
- AND the output MUST NOT repeat the same follow-up instruction in both a wizard-specific summary and a generic footer

#### Scenario: Generic footer does not contradict wizard summary

- GIVEN the user completes `agentsync init --wizard`
- AND the wizard-specific summary explains that apply and git review still remain
- WHEN command completion output is printed
- THEN no later generic footer text MUST imply that initialization is fully reconciled already
- AND no later generic footer text MUST omit or contradict the wizard-specific migration caveats
