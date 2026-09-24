# Repository maintenance

This guide records the collaboration and verification setup for `qunevo/apex`. GitHub-hosted settings are configured separately from the source files; a fork does not inherit those protections automatically.

## Community entry points

The [README](../readme.md), [contribution guide](../CONTRIBUTING.md), [support guide](../SUPPORT.md), [Code of Conduct](../CODE_OF_CONDUCT.md) and [security policy](../SECURITY.md) explain how to participate. Issues use structured bug and feature forms; Discussions host usage questions and early ideas. Vulnerabilities use GitHub private reporting. Repository content reporting is enabled for moderation.

Keep claims consistent with the [license](../LICENSE): APEX is source available and experimental. Do not add an OSI-open-source badge, imply production certification or publish unfinished commercial or contributor contracts. Resolve contribution rights before merging external work.

## Pull requests and branch protection

The `Protect main` ruleset targets the default branch, currently `main`. It requires a pull request, one approving review, approval of the latest reviewable push by another person and resolution of review conversations. New reviewable commits invalidate old approvals. Force pushes and deletion are blocked, and the bypass list is empty. Repository administrators can edit settings, so organization ownership remains privileged.

Require the GitHub Actions check **Required checks** after the CI workflow has completed successfully for the setup pull request. Select the GitHub Actions integration as its source and require branches to be up to date before merging. This check aggregates the Rust matrix, integration tests and publication scan and fails if any prerequisite fails, is cancelled or is skipped. Do not bypass review or temporarily disable protections to land this setup.

CI runs on every pull request and on pushes to `main`, including documentation-only changes, so a required check is never left waiting because of a path filter. Workflows use read-only repository permissions, do not retain checkout credentials and use Actions pinned to full commit hashes. Fork workflows require approval for all external contributors. Review the proposed workflow and scripts before approving a run; execution approval is not code review.

## Automated checks

- Stable Rust formatting, Clippy, tests and release builds on Linux, Windows and macOS.
- MCP stdio, HTTP authentication/origin behavior, and browser integration against a freshly built Linux release executable.
- Publication scanning of a clean source checkout for common credentials, machine paths and disallowed artifacts. The scanner does not establish that every example is synthetic or audit Git history.

The workflow builds executables only for testing and does not publish binaries, containers or release artifacts. See the [operating guide](implementation.md) for local equivalents.

Dependabot checks Cargo, npm test dependencies and GitHub Actions weekly. Minor and patch dependency updates are grouped to reduce noise. Updates still require passing CI, maintainer review and normal merge rules; there is no automatic merge or bot approval workflow.

## GitHub settings to maintain

- Enable private vulnerability reporting, dependency alerts, Dependabot security updates, secret scanning and push protection.
- Keep workflow tokens read-only and prevent workflows from approving pull requests.
- Require full commit hashes for actions and restrict permitted external actions to GitHub-owned actions used by the workflow. Review the policy deliberately if a new third-party action is proposed.
- Keep organization base repository permissions at Read; grant additional access only when a specific maintainer role needs it. Periodically review owners, teams, installed apps and deploy keys. Use two-factor authentication on maintainer accounts.
- Delete merged topic branches automatically. Keep Issues and Discussions available for the documented support channels.

These settings are not a substitute for reviewing source, dependencies and secrets. Check their live state when auditing the repository.

## Releases

Follow [source publication](publication.md). Review the source contents and license, run the complete CI suite, record the exact revision and document user-visible changes and migration steps. Publish a version tag and release notes only for an intentional release. Do not infer full migration parity, optimality or production readiness from a passing build.

The source license does not generally authorize public executable distribution by third parties. Do not add automatic binary/container publishing or a package-registry release without resolving the applicable distribution rights and release scope.
