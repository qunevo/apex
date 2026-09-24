# Repository maintenance

This guide records the collaboration and verification setup for `qunevo/apex`. GitHub-hosted settings are configured separately from the source files; a fork does not inherit those protections automatically.

## Community entry points

The [README](../readme.md), [contribution guide](../CONTRIBUTING.md), [support guide](../SUPPORT.md), [Code of Conduct](../CODE_OF_CONDUCT.md) and [security policy](../SECURITY.md) explain how to participate. Issues use structured bug and feature forms; Discussions host usage questions and early ideas. Vulnerabilities use GitHub private reporting. Repository content reporting is enabled for moderation.

Keep claims consistent with the [license](../LICENSE): APEX is source available and experimental. Do not add an OSI-open-source badge, imply production certification or publish unfinished commercial or contributor contracts. Resolve contribution rights before merging external work.

## Pull requests and branch protection

The `Protect main` ruleset targets the default branch, currently `main`. It requires a pull request and resolution of review conversations. The repository owner has selected a maintainer-controlled merge process: the required approving-review count is zero, and approval of the latest push by a different person is not required. A maintainer may merge their own pull request after the required checks pass. Force pushes and deletion remain blocked, and the bypass list is empty. Repository administrators can edit settings, so organization ownership remains privileged.

The merge gate is the GitHub Actions check **Required checks**, with GitHub Actions selected as its source and branches required to be up to date before merging. This check aggregates the Rust matrix, integration tests and publication scan and fails if any prerequisite fails, is cancelled or is skipped. When restoring these settings on a fork, run the workflow successfully before selecting the required check. Do not bypass or temporarily disable these checks to merge a change.

An agent using a maintainer's credentials acts as that maintainer, not as an independent reviewer. An explicit merge instruction from the owner authorizes the agent to inspect the diff and CI results, resolve outstanding review conversations and merge the verified revision through the normal pull-request endpoint. It does not authorize fabricated approval reviews, a second identity, persistent credentials in the repository or unrelated changes to repository access. Optional reviews remain available; new reviewable commits invalidate earlier approvals. Seek independent review when a change's risk or contribution rights warrant it.

CI runs on every pull request and on pushes to `main`, including documentation-only changes, so a required check is never left waiting because of a path filter. Validation jobs use read-only repository permissions and do not retain checkout credentials. Actions are pinned to full commit hashes. Fork workflows require approval for all external contributors. Review the proposed workflow and scripts before approving a run; execution approval is not code review.

The publication check includes the wiki export tests and local documentation-link validation. The separate [wiki publication workflow](wiki-publication.md) publishes the explicit public-page allowlist after changes reach `main`. Only its publication job receives `contents: write` and retains ephemeral checkout authentication for the wiki push; pull requests cannot run that job. The built-in Actions token is used without an AI service or a reusable personal credential.

## Automated checks

- Stable Rust formatting, Clippy, tests and release builds on Linux, Windows and macOS.
- MCP stdio, HTTP authentication/origin behavior, and browser integration against a freshly built Linux release executable.
- Publication scanning of a clean source checkout for common credentials, machine paths and disallowed artifacts. The scanner does not establish that every example is synthetic or audit Git history.

The workflow builds executables only for testing and does not publish binaries, containers or release artifacts. See the [operating guide](implementation.md) for local equivalents.

Dependabot checks Cargo, npm test dependencies and GitHub Actions weekly. Minor and patch dependency updates are grouped to reduce noise. Updates still require passing CI, maintainer review and normal merge rules; there is no automatic merge or bot approval workflow.

## GitHub settings to maintain

- Enable private vulnerability reporting, dependency alerts, Dependabot security updates, secret scanning and push protection.
- Keep default workflow tokens read-only and prevent workflows from approving pull requests. The wiki publication job declares its narrowly scoped write exception in reviewed workflow code.
- Require full commit hashes for actions and restrict permitted external actions to GitHub-owned actions used by the workflow. Review the policy deliberately if a new third-party action is proposed.
- Keep organization base repository permissions at Read; grant additional access only when a specific maintainer role needs it. Periodically review owners, teams, installed apps and deploy keys. Use two-factor authentication on maintainer accounts.
- Delete merged topic branches automatically. Keep Issues and Discussions available for the documented support channels.

These settings are not a substitute for reviewing source, dependencies and secrets. Check their live state when auditing the repository.

## Releases

Follow [source publication](publication.md). Review the source contents and license, run the complete CI suite, record the exact revision and document user-visible changes and migration steps. Publish a version tag and release notes only for an intentional release. Do not infer full migration parity, optimality or production readiness from a passing build.

The source license does not generally authorize public executable distribution by third parties. Do not add automatic binary/container publishing or a package-registry release without resolving the applicable distribution rights and release scope.
