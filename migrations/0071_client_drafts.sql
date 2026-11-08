-- Client versions from the panel (docs/future/game-launcher.md 4.10, operator request of 08/11/2026): a
-- draft lists files to add, replace or remove at a game path; publishing it makes version N+1 from N plus
-- those changes, without rebuilding the client. The file bytes wait in the server's staging folder under
-- their SHA-256. One open draft at a time. The client builder service fills the draft from the published
-- panel content (jobs below); every change is in admin_audit_events.
CREATE TABLE client_drafts(
 id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 state TEXT NOT NULL DEFAULT 'open' CHECK(state IN ('open','published','discarded')),
 -- The published version the draft applies to; publishing refuses if the store moved on.
 base_version INTEGER NOT NULL CHECK(base_version >= 1),
 created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
 account_id UUID REFERENCES accounts(id),
 closed_at TIMESTAMPTZ,
 published_version INTEGER CHECK(published_version >= 2),
 CHECK((state = 'published') = (published_version IS NOT NULL))
);
CREATE UNIQUE INDEX client_drafts_one_open ON client_drafts((state)) WHERE state = 'open';

CREATE TABLE client_draft_items(
 draft_id BIGINT NOT NULL REFERENCES client_drafts(id) ON DELETE CASCADE,
 -- Game path, '/' separators; compared without case (the published spelling wins).
 path TEXT NOT NULL CHECK(char_length(path) BETWEEN 1 AND 260),
 action TEXT NOT NULL CHECK(action IN ('put','remove')),
 -- The new file (put): SHA-256 and size of the staged bytes.
 sha256 TEXT CHECK(sha256 ~ '^[0-9a-f]{64}$'),
 size BIGINT CHECK(size >= 0),
 -- The published file it replaces or removes (NULL for a new path).
 before_sha256 TEXT CHECK(before_sha256 ~ '^[0-9a-f]{64}$'),
 -- 'upload' (panel) or 'content' (client builder: the published panel content applied to the client).
 source TEXT NOT NULL CHECK(source IN ('upload','content')),
 added_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
 PRIMARY KEY(draft_id, path),
 CHECK((action = 'put') = (sha256 IS NOT NULL AND size IS NOT NULL)),
 CHECK(action = 'put' OR before_sha256 IS NOT NULL)
);
CREATE UNIQUE INDEX client_draft_items_path ON client_draft_items(draft_id, lower(path));

-- Work for the client builder service (polled with FOR UPDATE SKIP LOCKED).
CREATE TABLE client_build_jobs(
 id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 kind TEXT NOT NULL CHECK(kind IN ('content')),
 state TEXT NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','running','done','failed')),
 account_id UUID REFERENCES accounts(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
 started_at TIMESTAMPTZ,
 finished_at TIMESTAMPTZ,
 -- Short outcome for the panel (no secret, no path outside the client).
 message TEXT CHECK(char_length(message) <= 2000)
);
CREATE UNIQUE INDEX client_build_jobs_one_pending ON client_build_jobs((kind)) WHERE state IN ('queued','running');
