//! PostgreSQL `Store`. Each tenant-scoped operation runs in a transaction that
//! sets `apex.tenant_id`, so row-level security enforces isolation in addition
//! to the explicit tenant filters.
use apex_control::{
    Error, Result,
    model::*,
    store::{ClaimPolicy, Completion, Store},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sqlx::{
    PgPool, Postgres, Row, Transaction,
    postgres::{PgPoolOptions, PgRow},
};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

fn db(e: sqlx::Error) -> Error {
    Error::Store(e.to_string())
}
fn json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("record serializes")
}
fn decode<T: DeserializeOwned>(row: &PgRow, column: &str) -> Result<T> {
    let value: Value = row.try_get(column).map_err(db)?;
    serde_json::from_value(value).map_err(|e| Error::Store(format!("{column}: {e}")))
}
fn missing(kind: &str, id: impl std::fmt::Display) -> Error {
    Error::NotFound(format!("{kind} {id}"))
}
fn as_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}
fn as_i64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| Error::invalid("revision is out of range"))
}

fn scenario(row: &PgRow) -> Result<Scenario> {
    Ok(Scenario {
        id: ScenarioId(row.try_get("id").map_err(db)?),
        tenant: TenantId(row.try_get("tenant_id").map_err(db)?),
        name: row.try_get("name").map_err(db)?,
        engine: row.try_get("engine").map_err(db)?,
        current_revision: as_u64(row.try_get("current_revision").map_err(db)?),
        published_result: row
            .try_get::<Option<uuid::Uuid>, _>("published_result")
            .map_err(db)?
            .map(ResultId),
        created_by: row.try_get("created_by").map_err(db)?,
        created_at: row.try_get("created_at").map_err(db)?,
        updated_at: row.try_get("updated_at").map_err(db)?,
    })
}

fn revision(row: &PgRow) -> Result<Revision> {
    Ok(Revision {
        scenario: ScenarioId(row.try_get("scenario_id").map_err(db)?),
        tenant: TenantId(row.try_get("tenant_id").map_err(db)?),
        number: as_u64(row.try_get("number").map_err(db)?),
        content: decode(row, "content")?,
        content_hash: row.try_get("content_hash").map_err(db)?,
        note: row.try_get("note").map_err(db)?,
        author: row.try_get("author").map_err(db)?,
        created_at: row.try_get("created_at").map_err(db)?,
    })
}

fn run(row: &PgRow) -> Result<Run> {
    let state: String = row.try_get("state").map_err(db)?;
    Ok(Run {
        id: RunId(row.try_get("id").map_err(db)?),
        tenant: TenantId(row.try_get("tenant_id").map_err(db)?),
        scenario: ScenarioId(row.try_get("scenario_id").map_err(db)?),
        revision: as_u64(row.try_get("revision").map_err(db)?),
        engine: row.try_get("engine").map_err(db)?,
        options: row.try_get("options").map_err(db)?,
        state: RunState::parse(&state).ok_or_else(|| Error::Store(format!("run state {state}")))?,
        phase: row.try_get("phase").map_err(db)?,
        cancel_requested: row.try_get("cancel_requested").map_err(db)?,
        attempts: u32::try_from(row.try_get::<i32, _>("attempts").map_err(db)?).unwrap_or(0),
        worker: row.try_get("worker").map_err(db)?,
        lease_until: row.try_get("lease_until").map_err(db)?,
        diagnostics: decode(row, "diagnostics")?,
        result: row
            .try_get::<Option<uuid::Uuid>, _>("result_id")
            .map_err(db)?
            .map(ResultId),
        created_by: row.try_get("created_by").map_err(db)?,
        created_at: row.try_get("created_at").map_err(db)?,
        updated_at: row.try_get("updated_at").map_err(db)?,
    })
}

fn status(row: &PgRow) -> Result<ResultStatus> {
    let status: String = row.try_get("status").map_err(db)?;
    ResultStatus::parse(&status).ok_or_else(|| Error::Store(format!("result status {status}")))
}

fn result(row: &PgRow) -> Result<PlanResult> {
    Ok(PlanResult {
        id: ResultId(row.try_get("id").map_err(db)?),
        tenant: TenantId(row.try_get("tenant_id").map_err(db)?),
        scenario: ScenarioId(row.try_get("scenario_id").map_err(db)?),
        revision: as_u64(row.try_get("revision").map_err(db)?),
        run: RunId(row.try_get("run_id").map_err(db)?),
        provenance: decode(row, "provenance")?,
        validation: decode(row, "validation")?,
        metrics: decode(row, "metrics")?,
        view: decode(row, "view")?,
        schedule: row.try_get("schedule").map_err(db)?,
        status: status(row)?,
        decisions: decode(row, "decisions")?,
        created_at: row.try_get("created_at").map_err(db)?,
    })
}

fn summary(row: &PgRow) -> Result<ResultSummary> {
    Ok(ResultSummary {
        id: ResultId(row.try_get("id").map_err(db)?),
        scenario: ScenarioId(row.try_get("scenario_id").map_err(db)?),
        revision: as_u64(row.try_get("revision").map_err(db)?),
        run: RunId(row.try_get("run_id").map_err(db)?),
        status: status(row)?,
        valid: row.try_get("valid").map_err(db)?,
        metrics: decode(row, "metrics")?,
        created_at: row.try_get("created_at").map_err(db)?,
    })
}

impl PgStore {
    pub async fn connect(url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(url)
            .await
            .map_err(db)?;
        Ok(Self { pool })
    }
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Apply embedded migrations. Run with the owning role, not the application role.
    pub async fn migrate(&self) -> Result<()> {
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|e| Error::Store(e.to_string()))
    }

    /// Grant an application role the access it needs, without bypassing RLS.
    pub async fn grant_app_role(&self, role: &str) -> Result<()> {
        if role.is_empty() || !role.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(Error::invalid(
                "Role names may contain only letters, digits and _",
            ));
        }
        let statements = [
            format!("GRANT SELECT, INSERT ON tenants TO {role}"),
            format!(
                "GRANT SELECT, INSERT, UPDATE ON scenarios, revisions, runs, results TO {role}"
            ),
            format!(
                "GRANT EXECUTE ON FUNCTION apex_claim_run(text, timestamptz, timestamptz, integer, integer) TO {role}"
            ),
        ];
        for statement in statements {
            // The role name is validated above; GRANT cannot bind identifiers.
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&self.pool)
                .await
                .map_err(db)?;
        }
        Ok(())
    }

    async fn tenant_tx(&self, tenant: TenantId) -> Result<Transaction<'static, Postgres>> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("SELECT set_config('apex.tenant_id', $1, true)")
            .bind(tenant.to_string())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        Ok(tx)
    }
}

#[async_trait]
impl Store for PgStore {
    async fn ensure_tenant(&self, tenant: &Tenant) -> Result<()> {
        sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
            .bind(tenant.id.0)
            .bind(&tenant.name)
            .execute(&self.pool)
            .await
            .map_err(db)?;
        Ok(())
    }

    async fn insert_scenario(&self, s: &Scenario, first: &Revision) -> Result<()> {
        let mut tx = self.tenant_tx(s.tenant).await?;
        sqlx::query(
            "INSERT INTO scenarios (id, tenant_id, name, engine, current_revision, published_result,
                created_by, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(s.id.0)
        .bind(s.tenant.0)
        .bind(&s.name)
        .bind(&s.engine)
        .bind(as_i64(s.current_revision)?)
        .bind(s.published_result.map(|r| r.0))
        .bind(&s.created_by)
        .bind(s.created_at)
        .bind(s.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        insert_revision(&mut tx, first).await?;
        tx.commit().await.map_err(db)
    }

    async fn list_scenarios(&self, tenant: TenantId) -> Result<Vec<Scenario>> {
        let mut tx = self.tenant_tx(tenant).await?;
        let rows = sqlx::query("SELECT * FROM scenarios WHERE tenant_id = $1 ORDER BY created_at")
            .bind(tenant.0)
            .fetch_all(&mut *tx)
            .await
            .map_err(db)?;
        rows.iter().map(scenario).collect()
    }

    async fn get_scenario(&self, tenant: TenantId, id: ScenarioId) -> Result<Option<Scenario>> {
        let mut tx = self.tenant_tx(tenant).await?;
        let row = sqlx::query("SELECT * FROM scenarios WHERE tenant_id = $1 AND id = $2")
            .bind(tenant.0)
            .bind(id.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
        row.as_ref().map(scenario).transpose()
    }

    async fn append_revision(
        &self,
        tenant: TenantId,
        expected: u64,
        r: &Revision,
    ) -> Result<Scenario> {
        let mut tx = self.tenant_tx(tenant).await?;
        let updated = sqlx::query(
            "UPDATE scenarios SET current_revision = $3, updated_at = $4
             WHERE tenant_id = $1 AND id = $2 AND current_revision = $5 AND $3 = $5 + 1
             RETURNING *",
        )
        .bind(tenant.0)
        .bind(r.scenario.0)
        .bind(as_i64(r.number)?)
        .bind(r.created_at)
        .bind(as_i64(expected)?)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let Some(row) = updated else {
            let current: Option<i64> = sqlx::query_scalar(
                "SELECT current_revision FROM scenarios WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant.0)
            .bind(r.scenario.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
            return Err(match current {
                None => missing("scenario", r.scenario),
                Some(c) => Error::Conflict(format!(
                    "Expected revision {expected}, current revision is {c}"
                )),
            });
        };
        insert_revision(&mut tx, r).await?;
        let s = scenario(&row)?;
        tx.commit().await.map_err(db)?;
        Ok(s)
    }

    async fn get_revision(
        &self,
        tenant: TenantId,
        id: ScenarioId,
        number: u64,
    ) -> Result<Option<Revision>> {
        let mut tx = self.tenant_tx(tenant).await?;
        let row = sqlx::query(
            "SELECT * FROM revisions WHERE tenant_id = $1 AND scenario_id = $2 AND number = $3",
        )
        .bind(tenant.0)
        .bind(id.0)
        .bind(as_i64(number)?)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        row.as_ref().map(revision).transpose()
    }

    async fn insert_run(&self, r: &Run) -> Result<()> {
        let mut tx = self.tenant_tx(r.tenant).await?;
        sqlx::query(
            "INSERT INTO runs (id, tenant_id, scenario_id, revision, engine, options, state, phase,
                cancel_requested, attempts, worker, lease_until, diagnostics, result_id, created_by,
                created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)",
        )
        .bind(r.id.0)
        .bind(r.tenant.0)
        .bind(r.scenario.0)
        .bind(as_i64(r.revision)?)
        .bind(&r.engine)
        .bind(&r.options)
        .bind(r.state.as_str())
        .bind(&r.phase)
        .bind(r.cancel_requested)
        .bind(i32::try_from(r.attempts).unwrap_or(i32::MAX))
        .bind(&r.worker)
        .bind(r.lease_until)
        .bind(json(&r.diagnostics))
        .bind(r.result.map(|x| x.0))
        .bind(&r.created_by)
        .bind(r.created_at)
        .bind(r.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        tx.commit().await.map_err(db)
    }

    async fn get_run(&self, tenant: TenantId, id: RunId) -> Result<Option<Run>> {
        let mut tx = self.tenant_tx(tenant).await?;
        let row = sqlx::query("SELECT * FROM runs WHERE tenant_id = $1 AND id = $2")
            .bind(tenant.0)
            .bind(id.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
        row.as_ref().map(run).transpose()
    }

    async fn list_runs(&self, tenant: TenantId, scenario: ScenarioId) -> Result<Vec<Run>> {
        let mut tx = self.tenant_tx(tenant).await?;
        let rows = sqlx::query(
            "SELECT * FROM runs WHERE tenant_id = $1 AND scenario_id = $2 ORDER BY created_at",
        )
        .bind(tenant.0)
        .bind(scenario.0)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?;
        rows.iter().map(run).collect()
    }

    async fn request_cancel(&self, tenant: TenantId, id: RunId, now: DateTime<Utc>) -> Result<Run> {
        let mut tx = self.tenant_tx(tenant).await?;
        let row = sqlx::query(
            "UPDATE runs SET
                state = CASE WHEN state = 'queued' THEN 'cancelled' ELSE state END,
                cancel_requested = cancel_requested OR state = 'running',
                updated_at = $3
             WHERE tenant_id = $1 AND id = $2 AND state IN ('queued', 'running')
             RETURNING *",
        )
        .bind(tenant.0)
        .bind(id.0)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let Some(row) = row else {
            let exists: Option<uuid::Uuid> =
                sqlx::query_scalar("SELECT id FROM runs WHERE tenant_id = $1 AND id = $2")
                    .bind(tenant.0)
                    .bind(id.0)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(db)?;
            return Err(match exists {
                None => missing("run", id),
                Some(_) => Error::Conflict(format!("Run {id} has already finished")),
            });
        };
        let r = run(&row)?;
        tx.commit().await.map_err(db)?;
        Ok(r)
    }

    async fn claim_run(
        &self,
        worker: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
        policy: ClaimPolicy,
    ) -> Result<Option<Run>> {
        let row = sqlx::query("SELECT * FROM apex_claim_run($1, $2, $3, $4, $5)")
            .bind(worker)
            .bind(now)
            .bind(lease_until)
            .bind(i32::try_from(policy.max_running_per_tenant).unwrap_or(i32::MAX))
            .bind(i32::try_from(policy.max_attempts).unwrap_or(i32::MAX))
            .fetch_optional(&self.pool)
            .await
            .map_err(db)?;
        row.as_ref().map(run).transpose()
    }

    async fn heartbeat(
        &self,
        r: &Run,
        worker: &str,
        phase: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
    ) -> Result<Option<Run>> {
        let mut tx = self.tenant_tx(r.tenant).await?;
        let row = sqlx::query(
            "UPDATE runs SET phase = $3, lease_until = $4, updated_at = $5
             WHERE id = $1 AND worker = $2 AND state = 'running'
             RETURNING *",
        )
        .bind(r.id.0)
        .bind(worker)
        .bind(phase)
        .bind(lease_until)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let current = row.as_ref().map(run).transpose()?;
        tx.commit().await.map_err(db)?;
        Ok(current)
    }

    async fn complete_run(
        &self,
        r: &Run,
        worker: &str,
        completion: Completion,
        now: DateTime<Utc>,
    ) -> Result<Run> {
        let mut tx = self.tenant_tx(r.tenant).await?;
        let owned: Option<uuid::Uuid> = sqlx::query_scalar(
            "SELECT id FROM runs WHERE id = $1 AND worker = $2 AND state = 'running' FOR UPDATE",
        )
        .bind(r.id.0)
        .bind(worker)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        if owned.is_none() {
            return Err(Error::Conflict(format!(
                "Worker {worker} no longer owns run {}",
                r.id
            )));
        }
        let (state, diagnostics, result_id) = match &completion {
            Completion::Succeeded(result) => {
                insert_result(&mut tx, result).await?;
                (RunState::Succeeded, Vec::new(), Some(result.id.0))
            }
            Completion::Failed(d) => (RunState::Failed, d.clone(), None),
            Completion::Cancelled => (RunState::Cancelled, Vec::new(), None),
        };
        let row = sqlx::query(
            "UPDATE runs SET state = $2, diagnostics = $3, result_id = $4, worker = NULL,
                lease_until = NULL, phase = NULL, updated_at = $5
             WHERE id = $1 RETURNING *",
        )
        .bind(r.id.0)
        .bind(state.as_str())
        .bind(json(&diagnostics))
        .bind(result_id)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        let finished = run(&row)?;
        tx.commit().await.map_err(db)?;
        Ok(finished)
    }

    async fn get_result(&self, tenant: TenantId, id: ResultId) -> Result<Option<PlanResult>> {
        let mut tx = self.tenant_tx(tenant).await?;
        let row = sqlx::query("SELECT * FROM results WHERE tenant_id = $1 AND id = $2")
            .bind(tenant.0)
            .bind(id.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
        row.as_ref().map(result).transpose()
    }

    async fn list_results(
        &self,
        tenant: TenantId,
        scenario: ScenarioId,
    ) -> Result<Vec<ResultSummary>> {
        let mut tx = self.tenant_tx(tenant).await?;
        let rows = sqlx::query(
            "SELECT id, scenario_id, revision, run_id, status, valid, metrics, created_at
             FROM results WHERE tenant_id = $1 AND scenario_id = $2 ORDER BY created_at",
        )
        .bind(tenant.0)
        .bind(scenario.0)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?;
        rows.iter().map(summary).collect()
    }

    async fn decide_result(
        &self,
        tenant: TenantId,
        id: ResultId,
        decision: &Decision,
    ) -> Result<PlanResult> {
        let mut tx = self.tenant_tx(tenant).await?;
        let row = sqlx::query(
            "UPDATE results SET status = $3, decisions = decisions || $4
             WHERE tenant_id = $1 AND id = $2 AND status = 'proposed'
             RETURNING *",
        )
        .bind(tenant.0)
        .bind(id.0)
        .bind(decision.status.as_str())
        .bind(Value::Array(vec![json(decision)]))
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let Some(row) = row else {
            let current = current_status(&mut tx, tenant, id).await?;
            return Err(Error::Conflict(format!(
                "Result {id} is {}, not proposed",
                current.as_str()
            )));
        };
        let r = result(&row)?;
        tx.commit().await.map_err(db)?;
        Ok(r)
    }

    async fn publish_result(
        &self,
        tenant: TenantId,
        id: ResultId,
        decision: &Decision,
    ) -> Result<PlanResult> {
        let mut tx = self.tenant_tx(tenant).await?;
        let row = sqlx::query(
            "SELECT r.status, r.revision, r.scenario_id, s.current_revision, s.published_result
             FROM results r JOIN scenarios s ON s.id = r.scenario_id
             WHERE r.tenant_id = $1 AND r.id = $2
             FOR UPDATE OF r, s",
        )
        .bind(tenant.0)
        .bind(id.0)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?
        .ok_or_else(|| missing("result", id))?;
        let current = status(&row)?;
        if current != ResultStatus::Approved {
            return Err(Error::Conflict(format!(
                "Result {id} is {}; only approved results can be published",
                current.as_str()
            )));
        }
        let (planned, latest): (i64, i64) = (
            row.try_get("revision").map_err(db)?,
            row.try_get("current_revision").map_err(db)?,
        );
        if planned != latest {
            return Err(Error::Conflict(format!(
                "Result {id} was planned for revision {planned}, but the scenario is at revision {latest}"
            )));
        }
        let scenario_id: uuid::Uuid = row.try_get("scenario_id").map_err(db)?;
        let previous: Option<uuid::Uuid> = row.try_get("published_result").map_err(db)?;
        if let Some(previous) = previous {
            sqlx::query(
                "UPDATE results SET status = 'superseded' WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant.0)
            .bind(previous)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        }
        sqlx::query(
            "UPDATE scenarios SET published_result = $3, updated_at = $4
             WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant.0)
        .bind(scenario_id)
        .bind(id.0)
        .bind(decision.at)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        let row = sqlx::query(
            "UPDATE results SET status = 'published', decisions = decisions || $3
             WHERE tenant_id = $1 AND id = $2 RETURNING *",
        )
        .bind(tenant.0)
        .bind(id.0)
        .bind(Value::Array(vec![json(decision)]))
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        let published = result(&row)?;
        tx.commit().await.map_err(db)?;
        Ok(published)
    }
}

async fn insert_revision(tx: &mut Transaction<'static, Postgres>, r: &Revision) -> Result<()> {
    sqlx::query(
        "INSERT INTO revisions (tenant_id, scenario_id, number, content, content_hash, note, author,
            created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(r.tenant.0)
    .bind(r.scenario.0)
    .bind(as_i64(r.number)?)
    .bind(json(&r.content))
    .bind(&r.content_hash)
    .bind(&r.note)
    .bind(&r.author)
    .bind(r.created_at)
    .execute(&mut **tx)
    .await
    .map_err(db)?;
    Ok(())
}

async fn insert_result(tx: &mut Transaction<'static, Postgres>, r: &PlanResult) -> Result<()> {
    sqlx::query(
        "INSERT INTO results (id, tenant_id, scenario_id, revision, run_id, provenance, validation,
            valid, metrics, view, schedule, status, decisions, created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
    )
    .bind(r.id.0)
    .bind(r.tenant.0)
    .bind(r.scenario.0)
    .bind(as_i64(r.revision)?)
    .bind(r.run.0)
    .bind(json(&r.provenance))
    .bind(json(&r.validation))
    .bind(r.validation.valid)
    .bind(json(&r.metrics))
    .bind(json(&r.view))
    .bind(&r.schedule)
    .bind(r.status.as_str())
    .bind(json(&r.decisions))
    .bind(r.created_at)
    .execute(&mut **tx)
    .await
    .map_err(db)?;
    Ok(())
}

async fn current_status(
    tx: &mut Transaction<'static, Postgres>,
    tenant: TenantId,
    id: ResultId,
) -> Result<ResultStatus> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM results WHERE tenant_id = $1 AND id = $2")
            .bind(tenant.0)
            .bind(id.0)
            .fetch_optional(&mut **tx)
            .await
            .map_err(db)?;
    let status = status.ok_or_else(|| missing("result", id))?;
    ResultStatus::parse(&status).ok_or_else(|| Error::Store(format!("result status {status}")))
}
