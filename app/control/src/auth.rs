//! Authenticated actors and role-based permissions, checked by every operation.
use crate::{error::Error, model::TenantId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Read scenarios, runs and results.
    Viewer,
    /// Edit scenarios and start or cancel runs.
    Planner,
    /// Approve, reject and publish results.
    Approver,
    /// All tenant operations.
    Admin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Read,
    EditScenario,
    Run,
    Decide,
    Publish,
}

impl Role {
    pub fn grants(self, permission: Permission) -> bool {
        use Permission::*;
        match self {
            Role::Admin => true,
            Role::Viewer => permission == Read,
            Role::Planner => matches!(permission, Read | EditScenario | Run),
            Role::Approver => matches!(permission, Read | Decide | Publish),
        }
    }
}

/// A person, script or agent acting within exactly one tenant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub tenant: TenantId,
    pub id: String,
    pub roles: Vec<Role>,
}

impl Actor {
    pub fn require(&self, permission: Permission) -> Result<(), Error> {
        if self.roles.iter().any(|r| r.grants(permission)) {
            Ok(())
        } else {
            Err(Error::Forbidden(format!(
                "Actor {} lacks permission {permission:?}",
                self.id
            )))
        }
    }
}

#[test]
fn roles_separate_planning_from_approval() {
    assert!(Role::Planner.grants(Permission::Run));
    assert!(!Role::Planner.grants(Permission::Publish));
    assert!(Role::Approver.grants(Permission::Publish));
    assert!(!Role::Approver.grants(Permission::EditScenario));
    assert!(!Role::Viewer.grants(Permission::Run));
}
