use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    ServiceRead,
    ServiceWrite,
    InstanceRead,
    InstanceWrite,
    UserManage,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum Role {
    Admin,
    Agent,
    Viewer,
}

impl Role {
    pub fn permissions(&self) -> HashSet<Permission> {
        match self {
            Role::Admin => [
                Permission::ServiceRead,
                Permission::ServiceWrite,
                Permission::InstanceRead,
                Permission::InstanceWrite,
                Permission::UserManage,
            ].into_iter().collect(),
            Role::Agent => [
                Permission::ServiceRead,
                Permission::InstanceRead,
                Permission::InstanceWrite,
            ].into_iter().collect(),
            Role::Viewer => [
                Permission::ServiceRead,
                Permission::InstanceRead,
            ].into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub common_name: String,
    pub organization: Option<String>,
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (Common Name)
    pub roles: Vec<Role>,
    pub exp: usize,
}
