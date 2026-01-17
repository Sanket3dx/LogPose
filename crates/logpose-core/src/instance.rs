use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::protocol::Protocol;
use crate::runtime::Runtime;
use crate::health::HealthStatus;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServiceInstance {
    pub id: Uuid,
    pub service_name: String,
    /// Network address (IP + port)
    pub address: SocketAddr,
    pub protocol: Protocol,
    pub runtime: Runtime,
    pub metadata: HashMap<String, String>,
    pub last_seen: u64,
    pub health: HealthStatus,
}

impl ServiceInstance {
    pub fn new(
        service_name: impl Into<String>,
        address: SocketAddr,
        protocol: Protocol,
        runtime: Runtime,
        last_seen: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            service_name: service_name.into(),
            address,
            protocol,
            runtime,
            metadata: HashMap::new(),
            last_seen,
            health: HealthStatus::Unknown,
        }
    }

    pub fn set_health(&mut self, health: HealthStatus) {
        self.health = health;
    }

    pub fn update_heartbeat(&mut self, timestamp: u64) {
        self.last_seen = timestamp;
    }

    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}
