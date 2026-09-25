# Repository maintenance

This guide defines collaboration and verification policy for `qunevo/apex`. GitHub-hosted settings are configured separately from source files; local edits and forks do not activate those protections automatically. Follow [developer workflow](developer-workflow.md) for skills, branch routing and bootstrap, and [versioning](versioning.md) for release semantics.

## Community entry points

The [README](../../readme.md), [contribution guide](../../CONTRIBUTING.md), [support guide](../../SUPPORT.md), [Code of Conduct](../../CODE_OF_CONDUCT.md) and [security policy](../../SECURITY.md) explain how to participate. Issues use structured bug and feature forms; Discussions host usage questions and early ideas. Vulnerabilities use GitHub private reporting. Repository content reporting is enabled for moderation.

Keep claims consistent with the [license](../../LICENSE): APEX is source available and experimental. Do not add an OSI-open-source badge, imply production certification or publish unfinished commercial or contributor contracts. Resolve contribution rights before merging external work.

## Pull requests and branch protection

The `Protect main` ruleset targets the default branch, currently `main`. It requires a pull request and resolution of review conversations. The repository owner has selected a maintainer-controlled merge process: the required approving-review count is zero, and approval of the latest push by a different person is not required. A maintainer may merge their own pull request after the required checks pass. Force pushes and deletion remain blocked, and the bypass list is empty. Repository administrators can edit settings, so organization ownership remains privileged.

Apply equivalent protection to `dev`. The merge gate is the GitHub Actions check **Required checks**, with GitHub Actions selected as its source and branches required to be up to date before merging. It aggregates scope detection, the Rust/integration matrix, publication checks and standalone application check. Only runtime jobs explicitly classified as unnecessary may be skipped; failed or cancelled required work fails the gate. When restoring settings on a fork, run the workflow successfully before selecting the required check. Never bypass these checks to merge a change.

An agent using a maintainer's credentials acts as that maintainer, not as an independent reviewer. An explicit merge instruction from the owner authorizes the agent to inspect the diff and CI results, resolve outstanding review conversations and merge the verified revision through the normal pull-request endpoint. It does not authorize fabricated approval reviews, a second identity, persistent credentials in the repository or unrelated changes to repository access. Optional reviews remain available; new reviewable commits invalidate earlier approvals. Seek independent review when a change's risk or contribution rights warrant it.

CI runs on every pull request to `dev` or `main` and pushes to either branch, including documentation-only changes. Job-level scope detection saves runtime work without suppressing the required gate. Validation jobs use read-only repository permissions and do not retain checkout credentials. Actions are pinned to full commit hashes. Fork workflow approval remains a GitHub setting; execution approval is not code review.

The publication check includes wiki export tests, local Markdown destinations, developer-skill consistency and version/lockfile validation. The separate [publication workflow](wiki-publication.md) waits for successful main push CI, publishes a source release when the version changed and publishes the explicit wiki allowlist. Only that job receives `contents: write`; pull requests cannot run it. The built-in Actions token avoids a reusable publishing credential.

## Automated checks

- Stable Rust Clippy, tests and release builds on Linux, Windows and macOS; formatting once on Linux.
- MCP stdio, HTTP authentication/origin behavior, and browser integration against a freshly built Linux release executable.
- Publication scanning of a clean source checkout for common credentials, machine paths and disallowed artifacts. The scanner does not establish that every example is synthetic or audit Git history.

The required standalone job also exports only `app/` into a fresh directory outside the checkout, builds it with its own lockfile and exercises scheduling, validation, embedded HTTP assets and MCP setup.

CI builds executables only for testing. Publication creates source tags and release notes, not binary/container/package-registry distributions. See the [operating guide](../../app/docs/implementation.md) for local verification.

Dependabot checks Cargo, npm test dependencies and GitHub Actions monthly against `dev`. Security updates target default `main` independently and use the hotfix path when the product changes. Minor and patch updates remain grouped. Integration requires passing CI and normal merge rules; an explicitly instructed agent can complete the process without additional GitHub UI clicks.

## GitHub settings to maintain

- Enable private vulnerability reporting, dependency alerts, Dependabot security updates, secret scanning and push protection.
- Keep default workflow tokens read-only and prevent workflows from approving pull requests. The wiki publication job declares its narrowly scoped write exception in reviewed workflow code.
- Require full commit hashes for actions and restrict permitted external actions to GitHub-owned actions used by the workflow. Review the policy deliberately if a new third-party action is proposed.
- Keep organization base repository permissions at Read; grant additional access only when a specific maintainer role needs it. Periodically review owners, teams, installed apps and deploy keys. Use two-factor authentication on maintainer accounts.
- Disable repository-wide automatic branch deletion so synchronization PRs cannot remove `dev`; agents may delete their own merged temporary branches. Keep Issues and Discussions available for support.

These settings are not a substitute for reviewing source, dependencies and secrets. Check their live state when auditing the repository.

## Releases

Follow [source publication](publication.md). Review the source contents and license, run the complete CI suite, record the exact revision and document user-visible changes and migration steps. Publish a version tag and release notes only for an intentional release. Do not infer full migration parity, optimality or production readiness from a passing build.

The source license does not generally authorize public executable distribution by third parties. Do not add automatic binary/container publishing or a package-registry release without resolving the applicable distribution rights and release scope.
