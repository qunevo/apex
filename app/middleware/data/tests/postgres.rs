//! Runs when APEX_TEST_DATABASE_URL points to a PostgreSQL server where the
//! user may create databases and roles, e.g. a local container:
//! `docker run --rm -e POSTGRES_PASSWORD=apex -p 55432:5432 postgres:17-alpine`
//! with `APEX_TEST_DATABASE_URL=postgres://postgres:apex@127.0.0.1:55432/postgres`.
use apex_control::{
    Role, ScenarioContent, Store, Tenant, TenantId,
    ops::CreateScenario,
    testing::{FakeEngine, actor, conformance, content},
};
use apex_control_postgres::PgStore;
use sqlx::{Connection, PgConnection, Row};
use std::sync::Arc;

fn with_database(url: &str, database: &str, user: Option<(&str, &str)>) -> String {
    let (base, _) = url.rsplit_once('/').expect("database URL has a path");
    let base = match user {
        Some((name, password)) => {
            let (scheme, rest) = base.split_once("://").unwrap();
            let host = rest.rsplit_once('@').map_or(rest, |(_, h)| h);
            format!("{scheme}://{name}:{password}@{host}")
        }
        None => base.to_string(),
    };
    format!("{base}/{database}")
}

/// Test-only DDL with generated identifiers; DDL cannot bind identifiers.
async fn exec(conn: &mut PgConnection, sql: String) {
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .execute(conn)
        .await
        .unwrap();
}

#[tokio::test]
async fn postgres_store_conforms_and_enforces_rls() {
    let Ok(url) = std::env::var("APEX_TEST_DATABASE_URL") else {
        eprintln!("skipped: APEX_TEST_DATABASE_URL is not set");
        return;
    };
    let suffix = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
    let (database, role) = (format!("apex_test_{suffix}"), format!("apex_app_{suffix}"));
    let mut admin = PgConnection::connect(&url).await.unwrap();
    exec(&mut admin, format!("CREATE DATABASE {database}")).await;
    exec(
        &mut admin,
        format!("CREATE ROLE {role} LOGIN PASSWORD 'apex' NOSUPERUSER NOBYPASSRLS"),
    )
    .await;

    let owner = PgStore::connect(&with_database(&url, &database, None), 2)
        .await
        .unwrap();
    owner.migrate().await.unwrap();
    owner.grant_app_role(&role).await.unwrap();
    let app_url = with_database(&url, &database, Some((&role, "apex")));
    let app = Arc::new(PgStore::connect(&app_url, 4).await.unwrap());

    conformance(app.clone()).await;

    // Row-level security holds even for queries without tenant filters.
    let (a, b) = (TenantId::new(), TenantId::new());
    for (id, name) in [(a, "A"), (b, "B")] {
        app.ensure_tenant(&Tenant {
            id,
            name: name.into(),
        })
        .await
        .unwrap();
    }
    let engines = apex_control::EngineRegistry::default().with(Arc::new(FakeEngine));
    let control = apex_control::Control::new(app.clone(), engines);
    let create = |c: ScenarioContent| CreateScenario {
        customization: None,
        name: "RLS".into(),
        engine: "fake".into(),
        content: c,
        note: None,
    };
    let (in_a, _) = control
        .create_scenario(&actor(a, &[Role::Planner]), create(content(&[1])))
        .await
        .unwrap();

    let mut raw = PgConnection::connect(&app_url).await.unwrap();
    let visible = async |raw: &mut PgConnection, tenant: Option<TenantId>| -> i64 {
        let mut tx = raw.begin().await.unwrap();
        if let Some(t) = tenant {
            sqlx::query("SELECT set_config('apex.tenant_id', $1, true)")
                .bind(t.to_string())
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        let count: i64 = sqlx::query("SELECT count(*) AS n FROM scenarios WHERE id = $1")
            .bind(in_a.id.0)
            .fetch_one(&mut *tx)
            .await
            .unwrap()
            .get("n");
        tx.rollback().await.unwrap();
        count
    };
    assert_eq!(visible(&mut raw, None).await, 0);
    assert_eq!(visible(&mut raw, Some(b)).await, 0);
    assert_eq!(visible(&mut raw, Some(a)).await, 1);

    // Writing another tenant's row is rejected by the policy check.
    let mut tx = raw.begin().await.unwrap();
    sqlx::query("SELECT set_config('apex.tenant_id', $1, true)")
        .bind(b.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let forged = sqlx::query(
        "INSERT INTO scenarios (id, tenant_id, name, engine, current_revision, created_by,
            created_at, updated_at) VALUES ($1, $2, 'x', 'fake', 1, 'x', now(), now())",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(a.0)
    .execute(&mut *tx)
    .await;
    assert!(forged.is_err());
    drop(tx);

    drop(raw);
    app.pool().close().await;
    owner.pool().close().await;
    exec(&mut admin, format!("DROP DATABASE {database} WITH (FORCE)")).await;
    exec(&mut admin, format!("DROP ROLE {role}")).await;
}
