use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum Protocol {
    Http,
    Https,
    Tcp,
    Grpc,
    Udp,
    Custom(String),
}
