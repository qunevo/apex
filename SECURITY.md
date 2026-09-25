# Security policy

## Report a vulnerability privately

Use [GitHub private vulnerability reporting](https://github.com/qunevo/apex/security/advisories/new) for suspected security vulnerabilities. Do not open a public issue or discussion with exploit details, credentials or private data. If the private reporting form is unavailable, use the repository's Security tab to check the current reporting instructions before sharing details.

Include the affected version or commit, operating system, relevant configuration, potential impact and a minimal synthetic reproduction. Describe any mitigation you have found. Do not include real customer data, live access tokens or unnecessary personal information. Keep testing within systems you own or are authorized to assess.

Maintainers will assess the report and coordinate remediation and disclosure through the private report. This project does not promise a response deadline, a bug bounty or commercial incident-response service.

## Maintenance scope

APEX is experimental. Security fixes are developed against the current default branch; older versions do not have a guaranteed backport or long-term support policy. Include the exact revision in reports so maintainers can reproduce the issue. See [releases](https://github.com/qunevo/apex/releases) for any published updates and [publication guidance](dev/docs/publication.md) for release checks.

## Deployment responsibilities

The scheduler stores local artifacts and does not provide tenant isolation or per-user identity. Keep the server bound to loopback unless you have configured and reviewed authentication, origin checks and network access. Follow the [agent integration guide](app/docs/agent-integration.md) before exposing HTTP endpoints. Self-hosting, data protection and backups remain the operator's responsibility under the license.

Never commit `.env` files, tokens, customer datasets, `.apex` stores or machine-specific MCP configuration. Revoke and replace an exposed credential; removing it from the latest commit does not remove it from Git history. Secret scanning and the publication scanner are additional checks, not proof that a repository is free of sensitive data.
