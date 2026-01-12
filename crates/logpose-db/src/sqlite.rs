use rusqlite::{params, Connection, Result as SqlResult};
use serde_json;
use uuid::Uuid;

use logpose_core::{Service, ServiceInstance, Protocol, Runtime, HealthStatus, RegistryError, RegistryStore};

pub struct DbRegistry {
    conn: Connection,
}

impl DbRegistry {
    pub fn new(path: &str) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> SqlResult<()> {
        self.conn.execute_batch(
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
            "
        )?;
        Ok(())
    }
}

impl RegistryStore for DbRegistry {
    fn add_service(&self, service: &Service) -> Result<(), RegistryError> {
        let metadata = serde_json::to_string(&service.metadata).unwrap_or_default();
        self.conn.execute(
            "INSERT OR REPLACE INTO services (code, name, description, metadata) VALUES (?1, ?2, ?3, ?4)",
            params![service.code, service.name, service.description, metadata]
        ).map_err(|_| RegistryError::DuplicateInstance)?;
        Ok(())
    }

    fn add_instance(&self, instance: &ServiceInstance) -> Result<(), RegistryError> {
        let metadata = serde_json::to_string(&instance.metadata).unwrap_or_default();
        self.conn.execute(
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
        let mut stmt = self.conn.prepare("SELECT code, name, description, metadata FROM services WHERE code = ?1").map_err(|_| RegistryError::ServiceNotFound)?;
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
        let mut stmt = self.conn.prepare("SELECT id, address, protocol, runtime, metadata, health FROM instances WHERE service_code = ?1").map_err(|_| RegistryError::ServiceNotFound)?;
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

        rows.collect()
    }
}
