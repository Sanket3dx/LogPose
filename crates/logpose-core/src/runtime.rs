use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum Runtime {
    Vm {
        provider: Option<String>,
        id: Option<String>,
    },
    Container {
        container_id: String,
    },
    Serverless {
        function_name: String,
        region: Option<String>,
    },
    Custom(String),
}
