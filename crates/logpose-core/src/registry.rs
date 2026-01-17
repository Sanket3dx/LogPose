use crate::{Service, ServiceInstance, Identity, Role};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("Service not found")]
    ServiceNotFound,
    #[error("Instance not found")]
    InstanceNotFound,
    #[error("Duplicate instance")]
    DuplicateInstance,
}

pub trait RegistryStore {
    fn add_service(&self, service: &Service) -> Result<(), RegistryError>;
    fn add_instance(&self, instance: &ServiceInstance) -> Result<(), RegistryError>;
    fn get_service(&self, code: &str) -> Result<Service, RegistryError>;
    fn get_instances(&self, service_code: &str) -> Result<Vec<ServiceInstance>, RegistryError>;
    fn add_identity(&self, identity: &Identity) -> Result<(), RegistryError>;
    fn get_identity(&self, common_name: &str) -> Result<Identity, RegistryError>;
    fn add_role_to_identity(&self, common_name: &str, role: Role) -> Result<(), RegistryError>;
    fn update_instance_health(&self, id: &uuid::Uuid, health: crate::HealthStatus) -> Result<(), RegistryError>;
    fn get_all_instances(&self) -> Result<Vec<ServiceInstance>, RegistryError>;
    fn get_all_services(&self) -> Result<Vec<Service>, RegistryError>;
}
