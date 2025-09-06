# Soft & Hard Deletion Design

Date: 2025-09-05
Status: Draft (approved for implementation)
Author: (Generated)
Target Release: TBD

## 1. Overview
Add moderation capabilities to (Boards, Threads, Replies) enabling:
1. Soft delete (reversible hide) via `deleted_at` timestamp.
2. Hard delete (irreversible purge) honoring existing FK cascades.
3. Admin-only control for all delete/restore actions.
4. Ability for admins to optionally view & restore soft-deleted content; non-admins never see it.
5. Frontend UI actions & visual indicators.

Out of scope (future): audit log, deletion reasons, scheduled purge, moderator roles.

## 2. Data Model Changes
Add nullable `deleted_at TIMESTAMPTZ` to:
- `boards`
- `threads`
- `replies`

No cascading soft delete: children remain but become unreachable through normal traversal. Simpler, avoids large update storms, and remains reversible.

Rust model additions (`models.rs`):
- `Board { deleted_at: Option<DateTime<Utc>> }`
- `Thread { deleted_at: Option<DateTime<Utc>> }`
- `Reply { deleted_at: Option<DateTime<Utc>> }`

### Indexes (partial for active filtering)
```sql
CREATE INDEX idx_boards_not_deleted ON boards(id) WHERE deleted_at IS NULL;
CREATE INDEX idx_threads_board_active ON threads(board_id, bump_time DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_replies_thread_active ON replies(thread_id, created_at ASC) WHERE deleted_at IS NULL;
```
Rationale: maintain fast active listings without scanning deleted rows.

### Migration File
`migrations/20250905_000002_soft_delete.sql`
```sql
ALTER TABLE boards  ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE threads ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE replies ADD COLUMN deleted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_boards_not_deleted ON boards(id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_threads_board_active ON threads(board_id, bump_time DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_replies_thread_active ON replies(thread_id, created_at ASC) WHERE deleted_at IS NULL;
```

## 3. Semantics
Soft delete:
- Sets `deleted_at = now()`.
- Hidden from standard reads for non-admin users.
- Admins can include via `?include_deleted=1` (GET endpoints) or dedicated admin list patterns (future).
- Idempotent: repeating soft delete on already deleted returns success (no change).

Restore:
- Sets `deleted_at = NULL`.
- Idempotent.

Hard delete:
- Physical `DELETE` of row; FKs `ON DELETE CASCADE` remove dependents.
- Irreversible; restore not possible.

Parent Visibility Relationships:
- Soft-deleted board hides its threads implicitly because board listing and thread queries require non-deleted board unless admin with `include_deleted`.
- Soft-deleted thread hides replies (reply listing first checks thread visibility).
- Individual reply soft delete only hides that reply.

Access Rules:
- Non-admin access to soft-deleted entity => 404 (do not leak existence).
- Admin access: if `include_deleted=1`, entity returned with `deleted_at`; else also hidden.

## 4. API Design
### New Admin Endpoints (all require `Role::Admin`)
```
POST   /api/v1/admin/boards/{id}/soft-delete
POST   /api/v1/admin/boards/{id}/restore
DELETE /api/v1/admin/boards/{id}

POST   /api/v1/admin/threads/{id}/soft-delete
POST   /api/v1/admin/threads/{id}/restore
DELETE /api/v1/admin/threads/{id}

POST   /api/v1/admin/replies/{id}/soft-delete
POST   /api/v1/admin/replies/{id}/restore
DELETE /api/v1/admin/replies/{id}
```
Responses:
- Soft delete / restore: `200 { "status": "ok" }`
- Hard delete: `204 No Content` (success) or `404` if not found.

### Modified Existing GET Endpoints
Recognize `?include_deleted=1` ONLY if caller is admin:
- `GET /api/v1/boards`
- `GET /api/v1/boards/{id}/threads`
- `GET /api/v1/threads/{id}`
- `GET /api/v1/threads/{id}/replies`

Filtering logic:
- When `include_deleted` absent or user not admin: add `deleted_at IS NULL` predicate for that entity.
- Replies listing also enforces parent thread visibility.

### Error Codes
- 403: Non-admin attempting admin endpoints.
- 404: Soft-deleted resource requested by non-admin OR resource absent.
- 200/204: Successful operations.

### OpenAPI
- Add `deleted_at` to schemas (`nullable: true`).
- Document query parameter `include_deleted` for relevant endpoints (admin only).
- Tag new endpoints under `Admin Moderation`.

## 5. Repository Layer Changes
Extend trait signatures to include soft/hard operations and optional inclusion flag. (Breaking change acceptable now.)

```rust
async fn list_boards(&self, include_deleted: bool) -> RepoResult<Vec<Board>>;
async fn list_threads(&self, board_id: Id, include_deleted: bool) -> RepoResult<Vec<Thread>>;
async fn list_replies(&self, thread_id: Id, include_deleted: bool) -> RepoResult<Vec<Reply>>;

async fn soft_delete_board(&self, id: Id) -> RepoResult<()>;
async fn restore_board(&self, id: Id) -> RepoResult<()>;
async fn hard_delete_board(&self, id: Id) -> RepoResult<()>;
// Same pattern for thread & reply
```

SQL Patterns:
- Soft delete: `UPDATE <table> SET deleted_at = COALESCE(deleted_at, now()) WHERE id=$1` (affect 0 => NotFound)
- Restore: `UPDATE <table> SET deleted_at = NULL WHERE id=$1` (0 => NotFound)
- Hard delete: `DELETE FROM <table> WHERE id=$1` (0 => NotFound)
- List (active): add `AND deleted_at IS NULL` or use partial index by referencing only columns with predicate already.

## 6. Route Layer
- Add new handlers under `/api/v1/admin/*` verifying admin role early.
- Parse `include_deleted` from query; forward to repo calls.
- For create thread/reply: ensure parent not soft-deleted (explicit query or rely on foreign key? Need explicit check because parent exists but logically hidden). Strategy: fetch parent with `include_deleted=true`, reject if `deleted_at` not null.

## 7. Frontend Changes
### Data Types
Add `deleted_at?: string` to Board, Thread, Reply interfaces.

### Admin Detection
Reuse `/api/v1/auth/me` (role field). Provide `useAuth()` or existing hook extension including boolean `isAdmin`.

### Show Deleted Toggle
Per view (BoardsPage, BoardThreadsPage, ThreadPage) if `isAdmin` add a toggle (local state or URL param) controlling `include_deleted=1` when true.

### Actions UI
- Boards list row: If active -> buttons [Soft Delete] [Hard Delete]; if deleted -> [Restore] [Hard Delete].
- Threads list card: same actions.
- Thread detail page: banner if thread deleted: "Thread deleted (admin view)" + restore/hard delete actions.
- Replies: each reply shows subtle action menu (ellipsis) with soft-delete/restore/hard delete when admin. Deleted replies styled (faded, badge "Deleted").

### Styling
- Deleted entity: `opacity: 0.5`, grayscale, or line-through for text; small red badge.

### API Helpers (`lib/api.ts`)
```ts
export async function softDelete(kind: 'boards'|'threads'|'replies', id: number) {}
export async function restore(kind: 'boards'|'threads'|'replies', id: number) {}
export async function hardDelete(kind: 'boards'|'threads'|'replies', id: number) {}
```
Endpoints map to admin paths (note pluralization consistent with existing routes).

### React Query Cache Invalidation
- After board action: invalidate `['boards']`.
- Thread action: invalidate `['threads', boardId]` and for hard delete also `['thread', threadId]` if such a query exists.
- Reply action: invalidate `['replies', threadId]` and maybe `['thread', threadId]` for reply counts (future).

### Confirmation UX
- Hard delete: modal requiring confirm click (optionally typing the id — simple confirm for now).
- Soft delete & restore: immediate, with toast feedback.

## 8. Testing Strategy
### Backend Integration Tests
1. Soft delete board hides from non-admin list.
2. Admin with `include_deleted=1` sees board + `deleted_at`.
3. Restore board reappears as active.
4. Soft delete thread hides replies listing for non-admin (thread 404).
5. Access soft-deleted thread as non-admin => 404.
6. Hard delete thread removes it (subsequent get => 404; replies cascade removed).
7. Soft delete reply hides only that reply.
8. Admin listing with `include_deleted=1` shows both active & deleted replies.
9. Create thread under soft-deleted board => 404.
10. Idempotent soft delete (returns ok, timestamp unchanged or reused).

### Frontend Tests (Vitest + React Testing Library)
- Admin showDeleted toggle issues fetch with `?include_deleted=1`.
- Non-admin never sees deleted badge even if API might include (defensive: API won’t include it).
- Action buttons hidden for non-admin.
- After soft delete mutation, item style updates (optimistic) then refetch.

## 9. Performance Considerations
- Added partial indexes support existing access patterns (board->threads->replies) with minimal overhead.
- Soft delete avoids large cascading updates.
- Additional boolean filter vs timestamp comparison negligible.

## 10. Security & Authorization
- Enforcement server-side only (role check via existing JWT claims).
- Non-admin queries get no indication of deletion state (404). Avoids resource enumeration side-channel.
- Admin must explicitly opt-in to see deleted data via `include_deleted`.

## 11. Open Questions & Defaults
| Question | Default Chosen |
|----------|----------------|
| Allow moderators to soft delete? | No (Admins only) |
| Track deleter (deleted_by) | Not now |
| Hard delete response body | 204 No Content |
| Soft delete / restore response | 200 JSON {status:"ok"} |
| include_deleted param naming | `include_deleted` |

## 12. Future Enhancements (Not in Scope)
- `deleted_by` & `reason` columns.
- Moderation audit log table.
- Retention-based reaper job (auto hard delete after grace period).
- Bulk moderation endpoints.
- Pagination + filtering by deletion state explicitly.

## 13. Implementation Checklist
(Will be executed next phase.)

Database:
- [x] Add migration with columns & indexes.

Models:
- [x] Extend structs & derive schema.

Repo:
- [x] Update trait signatures.
- [x] Implement soft/hard/restore methods.
- [x] Adjust list queries with optional filter.
- [x] Parent existence + non-deleted checks on create (implemented in routes using repo getters).

Routes:
- [x] Add admin endpoints.
- [x] Parse `include_deleted` for admins.
- [ ] Update OpenAPI docs (pending annotation expansion for new admin endpoints & deleted_at fields).

Frontend:
- [x] Extend types.
- [x] Add API helpers.
 - [x] Add admin showDeleted toggle.
 - [x] Implement action buttons & confirmation modal (basic confirm dialogs).
 - [x] Visual styling for deleted state.

Testing:
- [ ] Backend integration tests (scenarios above).
- [ ] Frontend tests for visibility & actions.

Docs:
- [ ] Update top-level README and `docs/design.md` referencing this doc.
- [ ] Add API usage examples.

Validation:
- [x] `cargo build` passes (tests pending addition).
- [ ] `cargo test` pass (tests not yet written).
- [ ] `npm test` pass.
- [ ] Manual smoke: soft delete -> hidden -> restore -> visible -> hard delete -> gone.

## 14. Risk Assessment & Mitigations
| Risk | Mitigation |
|------|------------|
| Breaking change in repo trait | Single implementation easy to update |
| Forget to filter deleted parents | Centralize parent check in routes before create/list |
| UI confusion about permanence | Distinct labels + confirmation for hard delete |
| Race: restore vs hard delete | Hard delete final; restoration after 404 impossible — acceptable |

## 15. Summary
Introduce reversible soft deletion via `deleted_at` and irreversible hard deletion with admin-only controls, consistent filtering logic, minimal schema changes, and clear UI affordances. Design keeps future moderation extensions open while keeping current implementation lean.
