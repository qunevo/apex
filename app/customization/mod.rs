//! Package identity is separate from the core's native extension reference.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub id: String,
    pub version: String,
}
