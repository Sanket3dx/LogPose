use rusqlite::{params, Connection, Result as SqlResult};
use serde_json;
use uuid::Uuid;

use logpose_core::{Service, ServiceInstance, Protocol, Runtime, HealthStatus, RegistryError, RegistryStore, Identity, Role};

use std::sync::Mutex;

pub struct DbRegistry {
    conn: Mutex<Connection>,
}

impl DbRegistry {
    pub fn new(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS services (
                code TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                metadata TEXT
            );
            CREATE TABLE IF NOT EXISTS instances (
                id TEXT PRIMARY KEY,
                service_code TEXT NOT NULL,
                address TEXT NOT NULL,
                protocol TEXT NOT NULL,
                runtime TEXT NOT NULL,
                metadata TEXT,
                health TEXT NOT NULL,
                FOREIGN KEY(service_code) REFERENCES services(code)
            );
            CREATE TABLE IF NOT EXISTS identities (
                common_name TEXT PRIMARY KEY,
                organization TEXT,
                metadata TEXT
            );
            CREATE TABLE IF NOT EXISTS identity_roles (
                common_name TEXT,
                role TEXT,
                PRIMARY KEY(common_name, role),
                FOREIGN KEY(common_name) REFERENCES identities(common_name)
            );

            "
        )?;
        Ok(())
    }
}

impl RegistryStore for DbRegistry {
    fn add_service(&self, service: &Service) -> Result<(), RegistryError> {
        let metadata = serde_json::to_string(&service.metadata).unwrap_or_default();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO services (code, name, description, metadata) VALUES (?1, ?2, ?3, ?4)",
            params![service.code, service.name, service.description, metadata]
        ).map_err(|_| RegistryError::DuplicateInstance)?;
        Ok(())
    }

    fn add_instance(&self, instance: &ServiceInstance) -> Result<(), RegistryError> {
        let metadata = serde_json::to_string(&instance.metadata).unwrap_or_default();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO instances (id, service_code, address, protocol, runtime, metadata, health)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                instance.id.to_string(),
                instance.service_name,
                instance.address.to_string(),
                format!("{:?}", instance.protocol),
                format!("{:?}", instance.runtime),
                metadata,
                format!("{:?}", instance.health)
            ]
        ).map_err(|_| RegistryError::DuplicateInstance)?;
        Ok(())
    }

    fn get_service(&self, code: &str) -> Result<Service, RegistryError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT code, name, description, metadata FROM services WHERE code = ?1").map_err(|_| RegistryError::ServiceNotFound)?;
        let service = stmt.query_row([code], |row| {
            let code: String = row.get(0)?;
            let name: String = row.get(1)?;
            let description: String = row.get(2)?;
            let metadata_json: String = row.get(3)?;
            let metadata: std::collections::HashMap<String, String> = serde_json::from_str(&metadata_json).unwrap_or_default();

            Ok(Service {
                code,
                name,
                description,
                instances: Vec::new(),
                metadata,
            })
        }).map_err(|_| RegistryError::ServiceNotFound)?;
        Ok(service)
    }

    fn get_instances(&self, service_code: &str) -> Result<Vec<ServiceInstance>, RegistryError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, address, protocol, runtime, metadata, health FROM instances WHERE service_code = ?1").map_err(|_| RegistryError::ServiceNotFound)?;
        let rows = stmt.query_map([service_code], |row| {
            let id: String = row.get(0)?;
            let address: String = row.get(1)?;
            let protocol: String = row.get(2)?;
            let runtime: String = row.get(3)?;
            let metadata_json: String = row.get(4)?;
            let health_str: String = row.get(5)?;

            let address = address.parse().unwrap();
            let protocol = match protocol.as_str() {
                "Http" => Protocol::Http,
                "Https" => Protocol::Https,
                "Tcp" => Protocol::Tcp,
                "Grpc" => Protocol::Grpc,
                "Udp" => Protocol::Udp,
                other => Protocol::Custom(other.to_string()),
            };
            let runtime = match runtime.split(':').next().unwrap_or("") {
                "Vm" => Runtime::Vm { provider: None, id: None },
                "Container" => Runtime::Container { container_id: "".to_string() },
                "Serverless" => Runtime::Serverless { function_name: "".to_string(), region: None },
                other => Runtime::Custom(other.to_string()),
            };
            let metadata: std::collections::HashMap<String, String> = serde_json::from_str(&metadata_json).unwrap_or_default();
            let health = match health_str.as_str() {
                "Healthy" => HealthStatus::Healthy,
                "Unhealthy" => HealthStatus::Unhealthy,
                _ => HealthStatus::Unknown,
            };

            Ok(ServiceInstance {
                id: Uuid::parse_str(&id).unwrap(),
                service_name: service_code.to_string(),
                address,
                protocol,
                runtime,
                metadata,
                last_seen: 0,
                health,
            })
        }).map_err(|_| RegistryError::ServiceNotFound)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| RegistryError::ServiceNotFound)
    }

    fn add_identity(&self, identity: &Identity) -> Result<(), RegistryError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO identities (common_name, organization, metadata) VALUES (?1, ?2, ?3)",
            params![identity.common_name, identity.organization, "{}"]
        ).map_err(|_| RegistryError::DuplicateInstance)?;

        // For simplicity, we just clear and re-add roles. Better to use a transaction.
        conn.execute(
            "DELETE FROM identity_roles WHERE common_name = ?1",
            params![identity.common_name]
        ).map_err(|_| RegistryError::DuplicateInstance)?;

        for role in &identity.roles {
            self.add_role_to_identity(&identity.common_name, role.clone())?;
        }

        Ok(())
    }

    fn get_identity(&self, common_name: &str) -> Result<Identity, RegistryError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT common_name, organization FROM identities WHERE common_name = ?1")
            .map_err(|_| RegistryError::ServiceNotFound)?;
        
        let (cn, org) = stmt.query_row([common_name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        }).map_err(|_| RegistryError::ServiceNotFound)?;

        let mut stmt = conn.prepare("SELECT role FROM identity_roles WHERE common_name = ?1")
            .map_err(|_| RegistryError::ServiceNotFound)?;
        
        let roles = stmt.query_map([common_name], |row| {
            let role_str: String = row.get(0)?;
            Ok(match role_str.as_str() {
                "Admin" => Role::Admin,
                "Agent" => Role::Agent,
                "Viewer" => Role::Viewer,
                _ => Role::Viewer, // Default to viewer
            })
        }).map_err(|_| RegistryError::ServiceNotFound)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RegistryError::ServiceNotFound)?;

        Ok(Identity {
            common_name: cn,
            organization: org,
            roles,
        })
    }

    fn add_role_to_identity(&self, common_name: &str, role: Role) -> Result<(), RegistryError> {
        let role_str = match role {
            Role::Admin => "Admin",
            Role::Agent => "Agent",
            Role::Viewer => "Viewer",
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO identity_roles (common_name, role) VALUES (?1, ?2)",
            params![common_name, role_str]
        ).map_err(|_| RegistryError::DuplicateInstance)?;
        Ok(())
    }

    fn update_instance_health(&self, id: &uuid::Uuid, health: HealthStatus) -> Result<(), RegistryError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE instances SET health = ?1 WHERE id = ?2",
            params![
                format!("{:?}", health),
                id.to_string()
            ]
        ).map_err(|_| RegistryError::InstanceNotFound)?;
        Ok(())
    }

    fn get_all_instances(&self) -> Result<Vec<ServiceInstance>, RegistryError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, service_code, address, protocol, runtime, metadata, health FROM instances").map_err(|_| RegistryError::ServiceNotFound)?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let service_code: String = row.get(1)?;
            let address: String = row.get(2)?;
            let protocol: String = row.get(3)?;
            let runtime: String = row.get(4)?;
            let metadata_json: String = row.get(5)?;
            let health_str: String = row.get(6)?;

            let address = address.parse().unwrap();
            let protocol = match protocol.as_str() {
                "Http" => Protocol::Http,
                "Https" => Protocol::Https,
                "Tcp" => Protocol::Tcp,
                "Grpc" => Protocol::Grpc,
                "Udp" => Protocol::Udp,
                other => Protocol::Custom(other.to_string()),
            };
            let runtime = match runtime.split(':').next().unwrap_or("") {
                "Vm" => Runtime::Vm { provider: None, id: None },
                "Container" => Runtime::Container { container_id: "".to_string() },
                "Serverless" => Runtime::Serverless { function_name: "".to_string(), region: None },
                other => Runtime::Custom(other.to_string()),
            };
            let metadata: std::collections::HashMap<String, String> = serde_json::from_str(&metadata_json).unwrap_or_default();
            let health = match health_str.as_str() {
                "Healthy" => HealthStatus::Healthy,
                "Unhealthy" => HealthStatus::Unhealthy,
                _ => HealthStatus::Unknown,
            };

            Ok(ServiceInstance {
                id: Uuid::parse_str(&id).unwrap(),
                service_name: service_code,
                address,
                protocol,
                runtime,
                metadata,
                last_seen: 0,
                health,
            })
        }).map_err(|_| RegistryError::ServiceNotFound)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| RegistryError::ServiceNotFound)
    }
}
