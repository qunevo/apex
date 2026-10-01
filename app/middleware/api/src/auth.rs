//! Static bearer tokens mapped to tenant actors. Only token hashes are stored.
use apex_control::{Actor, Role, Tenant, TenantId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenEntry {
    /// Hex SHA-256 of the bearer token.
    pub sha256: String,
    pub tenant: TenantId,
    pub actor: String,
    pub roles: Vec<Role>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuthFile {
    pub tenants: Vec<Tenant>,
    pub tokens: Vec<TokenEntry>,
}

pub fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A new random bearer token. UUID v4 uses the operating system's CSPRNG.
pub fn generate_token() -> String {
    format!(
        "apx_{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[derive(Clone, Default)]
pub struct Tokens(HashMap<String, Actor>);

impl Tokens {
    pub fn load(path: &Path) -> Result<(Self, Vec<Tenant>), String> {
        let text = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: AuthFile =
            serde_json::from_slice(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_file(file)
    }

    pub fn from_file(file: AuthFile) -> Result<(Self, Vec<Tenant>), String> {
        let mut tokens = HashMap::new();
        for t in file.tokens {
            if !file.tenants.iter().any(|x| x.id == t.tenant) {
                return Err(format!("Token for {} names an unknown tenant", t.actor));
            }
            if t.roles.is_empty() {
                return Err(format!("Token for {} has no roles", t.actor));
            }
            let actor = Actor {
                tenant: t.tenant,
                id: t.actor,
                roles: t.roles,
            };
            if tokens
                .insert(t.sha256.to_ascii_lowercase(), actor)
                .is_some()
            {
                return Err("Duplicate token hash".into());
            }
        }
        Ok((Self(tokens), file.tenants))
    }

    pub fn actor(&self, bearer: &str) -> Option<&Actor> {
        self.0.get(&hash(bearer))
    }
}
