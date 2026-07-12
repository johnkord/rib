# Production Release Runbook

RIB production releases are manual maintenance operations. A push to `main` runs CI but does not deploy. The `Deploy to AKS` workflow must be dispatched explicitly after the exact commit has passed CI.

## Why This Release Needs Downtime

Migration `20260711000006_attachment_references.sql` removes the unique `images(hash)` contract required by older binaries. The Kubernetes Deployment therefore uses the `Recreate` strategy and one replica so old and new application versions never write against the changed schema at the same time.

After migration 006 has applied, do not roll back to an older image against the migrated database. Recovery is either:

1. Fix forward with a compatible image, or
2. Restore the pre-release PostgreSQL backup, restore object storage if it changed, and then deploy the prior image.

## Release Blockers

Do not dispatch the production workflow until all items are true:

- The release commit is on `main` and its `CI` workflow succeeded.
- PostgreSQL has an off-cluster backup with a recorded checksum/reference.
- MinIO/object storage has an off-cluster backup with a recorded checksum/reference.
- Both backups have passed a restore drill into an isolated environment.
- Azure Key Vault contains `rib-TRIPCODE-SECRET`, generated independently from `rib-JWT-SECRET` with at least 32 random characters.
- The GitHub Actions OIDC identity can read every `rib-*` secret used by `deploy-prod.yml`.
- The operator accepts a short outage while the single backend pod is recreated and migrations run.
- The operator has access to the previous image tag and the backup restore procedure.

For additional defense, configure a required reviewer on the GitHub `prod` environment. The repository cannot enforce that account-level setting.

## Backup Expectations

A disk snapshot alone is not a complete database backup. Prefer a PostgreSQL-native backup (`pg_dump` for this small deployment, or a tested physical/PITR process) and an object-store inventory/copy to storage outside the AKS cluster.

Record at least:

- UTC creation time
- Backup destination and immutable identifier
- SHA-256 checksum or provider integrity identifier
- PostgreSQL migration count (expected before this release: 5)
- Object count
- Restore-drill date and result

Pass the resulting identifier in the workflow's `backup_reference` input. The input is an audit record, not automatic proof that the backup is usable.

## Dispatch

1. Merge or push the release commit to `main`.
2. Wait for all `CI` jobs to succeed for that exact SHA.
3. Open Actions, select `Deploy to AKS`, and choose `Run workflow` on `main`.
4. Enter the verified backup reference.
5. Acknowledge downtime and the no-old-binary-rollback boundary.
6. Monitor image build/push, migration preflight, `Recreate` rollout, and smoke checks.

The workflow fails closed when CI is missing, required acknowledgements are false, Key Vault secrets are unavailable, live rows violate pending constraints, rollout fails, migration count is not 10, or public smoke checks fail.

The workflow deploys only the current `origin/main` tip. It will not deploy an older ancestor or an unmerged branch.

## Dependency Audit Note

`.cargo/audit.toml` ignores `RUSTSEC-2023-0071` only because SQLx records its optional MySQL driver's RSA dependency in `Cargo.lock` while RIB enables PostgreSQL only. The release graph has no RSA path, including with all features and all targets. Remove the exception if MySQL support is ever enabled.

## Post-Deploy Verification

Verify:

```bash
curl --fail https://rib.curlyquote.com/healthz
curl --fail https://rib.curlyquote.com/api/v1/auth/me
kubectl -n rib rollout status deployment/rib-backend-aks
kubectl -n rib get pods -l app=rib-backend -o wide
kubectl -n rib exec postgres-aks-0 -- \
  psql -U postgres -d rib -Atc 'select count(*) from _sqlx_migrations'
```

Expected anonymous `/auth/me` body: `null`. Expected migration count after this release: `10`.

Also exercise, with synthetic data:

- Allowlisted Discord login
- Bitcoin challenge issuance (verification only when a suitable test wallet is available)
- Thread and reply creation
- Arbitrary-file upload and safe download
- Moderator author lookup, ban, and unban
- Hard deletion and final object removal

Remove all synthetic records afterward.

## Recovery Decision

If the new pod never becomes ready:

1. Inspect pod logs and query `_sqlx_migrations`.
2. If the migration count is still `5`, redeploying the prior image is schema-compatible.
3. If migration 006 or later applied, do not deploy the prior image against that database.
4. Choose fix-forward or restore the recorded pre-release backup before restoring the prior image.

If the release served writes before a rollback decision, preserve and assess those writes before restoring a backup.

## Separate Operational Debt

Do not combine the application release with unrelated high-risk changes. Schedule separate, backed-up maintenance for:

- Rotating PostgreSQL and MinIO root credentials to least-privilege application identities
- Upgrading the old AKS/Kubernetes version
- Adding automated off-cluster backups and restore monitoring
- Adding NetworkPolicies and dependency-aware readiness
