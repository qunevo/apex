use apex_control::{Role, Store, Tenant, TenantId, memory::MemoryStore};
use apex_control_postgres::PgStore;
use apex_control_server::{
    auth::{AuthFile, TokenEntry, Tokens, generate_token, hash},
    engines, http, state, worker, workers,
};
use std::{env, path::PathBuf, sync::Arc};

const HELP: &str = "apex-control: APEX control platform server

Commands:
  init     Create an auth file with one tenant, an admin token for agents and
           a viewer token for display clients. Prints the tokens once.
  token    Add a token: --actor NAME --roles admin|planner|approver|viewer[,...] [--tenant ID]
  migrate  Apply database migrations [--app-role ROLE grants a non-owner role]
  serve    Run HTTP (/v1), MCP (/mcp), events (/v1/events) and workers

Options and environment:
  --auth FILE        APEX_CONTROL_AUTH          default .apex-control/auth.json
  --config FILE      APEX_CONFIG                optional customization allowlist and default
  --database URL     APEX_CONTROL_DATABASE_URL  PostgreSQL; omitted = in-memory (serve only)
  --bind ADDR        APEX_CONTROL_BIND          default 127.0.0.1:8780
  --max-request-bytes N APEX_CONTROL_MAX_REQUEST_BYTES default 2097152
  --workers N        APEX_CONTROL_WORKERS       default 2";

struct Args(Vec<String>);
impl Args {
    fn flag(&self, name: &str, env_name: &str) -> Option<String> {
        self.0
            .iter()
            .position(|x| x == name)
            .and_then(|i| self.0.get(i + 1).cloned())
            .or_else(|| env::var(env_name).ok().filter(|v| !v.is_empty()))
    }
    fn auth(&self) -> PathBuf {
        PathBuf::from(
            self.flag("--auth", "APEX_CONTROL_AUTH")
                .unwrap_or_else(|| ".apex-control/auth.json".into()),
        )
    }
}

type Failure = Box<dyn std::error::Error>;

fn write_auth(path: &PathBuf, file: &AuthFile) -> Result<(), Failure> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(file)?)?;
    Ok(())
}

fn roles(value: &str) -> Result<Vec<Role>, Failure> {
    value
        .split(',')
        .map(|r| {
            serde_json::from_value(serde_json::json!(r.trim()))
                .map_err(|_| format!("Unknown role {r}").into())
        })
        .collect()
}

fn add_token(file: &mut AuthFile, tenant: TenantId, actor: &str, roles: Vec<Role>) -> String {
    let token = generate_token();
    file.tokens.push(TokenEntry {
        sha256: hash(&token),
        tenant,
        actor: actor.into(),
        roles,
    });
    token
}

#[tokio::main]
async fn main() {
    if let Err(e) = run(Args(env::args().skip(1).collect())).await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), Failure> {
    match args.0.first().map(String::as_str).unwrap_or("help") {
        "init" => {
            let path = args.auth();
            if path.exists() {
                return Err(format!(
                    "{} already exists; use `token` to add access",
                    path.display()
                )
                .into());
            }
            let tenant = Tenant {
                id: TenantId::new(),
                name: args
                    .flag("--tenant-name", "APEX_CONTROL_TENANT_NAME")
                    .unwrap_or_else(|| "Local".into()),
            };
            let mut file = AuthFile {
                tenants: vec![tenant.clone()],
                tokens: Vec::new(),
            };
            let agent = add_token(&mut file, tenant.id, "local-agent", vec![Role::Admin]);
            let display = add_token(&mut file, tenant.id, "display", vec![Role::Viewer]);
            write_auth(&path, &file)?;
            println!(
                "Created {} for tenant {} ({}).",
                path.display(),
                tenant.name,
                tenant.id
            );
            println!("Store these tokens now; only their hashes are saved.");
            println!("  agent (admin):    {agent}");
            println!("  display (viewer): {display}");
        }
        "token" => {
            let path = args.auth();
            let mut file: AuthFile = serde_json::from_slice(&std::fs::read(&path)?)?;
            let actor = args.flag("--actor", "").ok_or("--actor is required")?;
            let roles = roles(&args.flag("--roles", "").ok_or("--roles is required")?)?;
            let tenant = match args.flag("--tenant", "") {
                Some(t) => t.parse()?,
                None if file.tenants.len() == 1 => file.tenants[0].id,
                None => return Err("--tenant is required when the file has several tenants".into()),
            };
            let token = add_token(&mut file, tenant, &actor, roles);
            Tokens::from_file(file.clone())?;
            write_auth(&path, &file)?;
            println!("{token}");
        }
        "migrate" => {
            let url = args
                .flag("--database", "APEX_CONTROL_DATABASE_URL")
                .ok_or("A database URL is required")?;
            let store = PgStore::connect(&url, 2).await?;
            store.migrate().await?;
            if let Some(role) = args.flag("--app-role", "") {
                store.grant_app_role(&role).await?;
            }
            println!("Migrations applied.");
        }
        "serve" => {
            let max_request_bytes = args
                .flag("--max-request-bytes", "APEX_CONTROL_MAX_REQUEST_BYTES")
                .map_or(Ok(http::DEFAULT_MAX_REQUEST_BYTES), |value| {
                    value.parse::<usize>()
                })?;
            if max_request_bytes == 0 {
                return Err("--max-request-bytes must be positive".into());
            }
            let (tokens, tenants) = Tokens::load(&args.auth())?;
            let store: Arc<dyn Store> = match args.flag("--database", "APEX_CONTROL_DATABASE_URL") {
                Some(url) => Arc::new(PgStore::connect(&url, 16).await?),
                None => {
                    eprintln!(
                        "No database configured: using an in-memory store; data is lost on exit."
                    );
                    Arc::new(MemoryStore::new())
                }
            };
            for tenant in &tenants {
                store.ensure_tenant(tenant).await?;
            }
            let engines = engines();
            let mut state = state(store, engines.clone(), tokens);
            if let Some(path) = args.flag("--config", "APEX_CONFIG") {
                let config = apex_control::packages::Config::load(std::path::Path::new(&path))?;
                state.control = state.control.with_packages(config);
            }
            let count: usize = args
                .flag("--workers", "APEX_CONTROL_WORKERS")
                .map_or(Ok(2), |v| v.parse())?;
            let host = format!(
                "{}-{}",
                env::var("HOSTNAME").unwrap_or_else(|_| "local".into()),
                std::process::id()
            );
            workers::spawn(
                worker(&state, engines, &host),
                count,
                state.events.clone(),
                state.wake.clone(),
            );
            let bind = args
                .flag("--bind", "APEX_CONTROL_BIND")
                .unwrap_or_else(|| "127.0.0.1:8780".into());
            let listener = tokio::net::TcpListener::bind(&bind).await?;
            println!(
                "apex-control listening on http://{bind} (HTTP /v1, MCP /mcp, events /v1/events)"
            );
            axum::serve(
                listener,
                http::router_with_body_limit(state, max_request_bytes),
            )
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        }
        _ => println!("{HELP}"),
    }
    Ok(())
}
