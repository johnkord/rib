# Security Policy

RIB is pre-1.0 software. Security fixes are applied to the current `main` branch; older commits and unreleased snapshots are not maintained as separate supported versions.

## Reporting A Vulnerability

Do not open a public issue for a suspected vulnerability.

Use [GitHub private vulnerability reporting](https://github.com/johnkord/rib/security/advisories/new) when available. If that channel is unavailable, contact the operator using the details on the deployed instance's About page and clearly mark the message as a private security report.

Include enough information to reproduce and assess the issue:

- Affected endpoint, component, commit, or deployment mode
- Preconditions and required privileges
- Reproduction steps or a minimal proof of concept
- Expected and observed behavior
- Likely impact
- Any suggested mitigation

Do not include real user data, private keys, access tokens, or production credentials. Use synthetic accounts and data whenever possible.

## Response

The maintainer will aim to acknowledge a complete report within seven days. Remediation timing depends on severity, exploitability, and release risk. A coordinated disclosure date will be agreed with the reporter when public disclosure is appropriate.

## Research Guidelines

Good-faith research should avoid:

- Privacy violations or access to data that is not your own
- Service disruption, resource exhaustion, or destructive testing
- Uploading illegal or harmful material
- Social engineering
- Automated scanning of third-party deployments without operator permission

Stop testing and report promptly if you encounter sensitive data or evidence of active exploitation.

## Operational Security

Public operators should follow the production checklist in [README.md](README.md), including least-privilege credentials, secure cookies, upload isolation, tested backups, dependency checks, and a single backend replica until ephemeral state is distributed.
