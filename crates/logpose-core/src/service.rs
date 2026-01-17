use std::collections::HashMap;
use crate::instance::ServiceInstance;

#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub code: String,
    pub description: String,
    pub instances: Vec<ServiceInstance>,
    pub metadata: HashMap<String, String>,
}

impl Service {
    pub fn new(
        name: impl Into<String>,
        code: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            code: code.into(),
            description: description.into(),
            instances: Vec::new(),
            metadata: HashMap::new(),
        }
    }
    pub fn add_instance(&mut self, instance: ServiceInstance) {
        self.instances.push(instance);
    }

    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}
