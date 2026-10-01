-- APEX control platform schema. Every tenant-owned table carries tenant_id and
-- forced row-level security keyed by the transaction setting apex.tenant_id.
-- Run migrations as the owning role; let the application connect as a separate
-- role without BYPASSRLS (see grant_app_role).

CREATE TABLE tenants (
    id uuid PRIMARY KEY,
    name text NOT NULL
);

CREATE TABLE scenarios (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenants (id),
    name text NOT NULL,
    engine text NOT NULL,
    current_revision bigint NOT NULL CHECK (current_revision >= 1),
    published_result uuid,
    created_by text NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    UNIQUE (tenant_id, id)
);

CREATE TABLE revisions (
    tenant_id uuid NOT NULL,
    scenario_id uuid NOT NULL,
    number bigint NOT NULL CHECK (number >= 1),
    content jsonb NOT NULL,
    content_hash text NOT NULL,
    note text,
    author text NOT NULL,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (scenario_id, number),
    FOREIGN KEY (tenant_id, scenario_id) REFERENCES scenarios (tenant_id, id)
);

CREATE TABLE runs (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    scenario_id uuid NOT NULL,
    revision bigint NOT NULL,
    engine text NOT NULL,
    options jsonb NOT NULL,
    state text NOT NULL CHECK (state IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    phase text,
    cancel_requested boolean NOT NULL DEFAULT false,
    attempts integer NOT NULL DEFAULT 0,
    worker text,
    lease_until timestamptz,
    diagnostics jsonb NOT NULL DEFAULT '[]',
    result_id uuid,
    created_by text NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    FOREIGN KEY (scenario_id, revision) REFERENCES revisions (scenario_id, number),
    FOREIGN KEY (tenant_id, scenario_id) REFERENCES scenarios (tenant_id, id)
);
CREATE INDEX runs_scenario ON runs (tenant_id, scenario_id, created_at);
CREATE INDEX runs_open ON runs (created_at) WHERE state IN ('queued', 'running');

CREATE TABLE results (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    scenario_id uuid NOT NULL,
    revision bigint NOT NULL,
    run_id uuid NOT NULL REFERENCES runs (id),
    provenance jsonb NOT NULL,
    validation jsonb NOT NULL,
    valid boolean NOT NULL,
    metrics jsonb NOT NULL,
    view jsonb NOT NULL,
    schedule jsonb NOT NULL,
    status text NOT NULL CHECK (status IN ('proposed', 'approved', 'rejected', 'published', 'superseded')),
    decisions jsonb NOT NULL DEFAULT '[]',
    created_at timestamptz NOT NULL,
    FOREIGN KEY (scenario_id, revision) REFERENCES revisions (scenario_id, number),
    FOREIGN KEY (tenant_id, scenario_id) REFERENCES scenarios (tenant_id, id)
);
CREATE INDEX results_scenario ON results (tenant_id, scenario_id, created_at);
-- At most one published result per scenario.
CREATE UNIQUE INDEX results_one_published ON results (scenario_id) WHERE status = 'published';

CREATE FUNCTION apex_current_tenant() RETURNS uuid
    LANGUAGE sql STABLE
    AS $$ SELECT nullif(current_setting('apex.tenant_id', true), '')::uuid $$;

DO $$
DECLARE t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['scenarios', 'revisions', 'runs', 'results'] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY tenant_isolation ON %I USING (tenant_id = apex_current_tenant()) '
            'WITH CHECK (tenant_id = apex_current_tenant())', t);
    END LOOP;
END $$;

-- The only cross-tenant operation: workers claim the oldest eligible run while
-- each tenant stays below its running limit. Abandoned runs whose lease expired
-- are retried, cancelled when requested, or failed after max_attempts. It runs
-- as the function owner; FORCE above still applies to owners, so the owner must
-- be a role permitted to bypass RLS (the migrating superuser in a default setup).
CREATE FUNCTION apex_claim_run(
    p_worker text,
    p_now timestamptz,
    p_lease timestamptz,
    p_max_running integer,
    p_max_attempts integer
) RETURNS SETOF runs
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    claimed runs;
BEGIN
    UPDATE runs SET
        state = CASE WHEN cancel_requested THEN 'cancelled' ELSE 'failed' END,
        diagnostics = CASE WHEN cancel_requested THEN diagnostics ELSE
            '[{"code":"ATTEMPTS_EXHAUSTED","message":"The run was abandoned by its workers too often"}]'::jsonb END,
        worker = NULL, lease_until = NULL, phase = NULL, updated_at = p_now
    WHERE state = 'running' AND lease_until < p_now
      AND (cancel_requested OR attempts >= p_max_attempts);

    SELECT * INTO claimed FROM runs c
    WHERE (c.state = 'queued' OR (c.state = 'running' AND c.lease_until < p_now))
      AND (SELECT count(*) FROM runs b
           WHERE b.tenant_id = c.tenant_id AND b.state = 'running'
             AND b.lease_until >= p_now) < p_max_running
    ORDER BY c.created_at
    LIMIT 1
    FOR UPDATE SKIP LOCKED;
    IF NOT FOUND THEN
        RETURN;
    END IF;

    UPDATE runs SET state = 'running', attempts = attempts + 1, worker = p_worker,
        lease_until = p_lease, phase = 'claimed', updated_at = p_now
    WHERE id = claimed.id
    RETURNING * INTO claimed;
    RETURN NEXT claimed;
END $$;
REVOKE ALL ON FUNCTION apex_claim_run(text, timestamptz, timestamptz, integer, integer) FROM PUBLIC;
