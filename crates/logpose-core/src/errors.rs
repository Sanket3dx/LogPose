/// Domain errors for LogPose core
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Service not found")]
    ServiceNotFound,

    #[error("Instance not found")]
    InstanceNotFound,

    #[error("Duplicate instance")]
    DuplicateInstance,
}
