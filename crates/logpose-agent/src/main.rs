use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use reqwest::Client;

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<serde_json::Value>,
}

struct AgentState {
    client: Client,
    server_url: String,
    token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    
    let state = Arc::new(AgentState {
        client: Client::new(),
        server_url: std::env::var("LOGPOSE_SERVER").unwrap_or_else(|_| "http://localhost:3000".to_string()),
        token: std::env::var("LOGPOSE_TOKEN").ok(),
    });

    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();

    while handle.read_line(&mut line)? > 0 {
        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => {
                line.clear();
                continue;
            }
        };

        let res = handle_request(req, state.clone()).await;
        if let Some(res_val) = res {
            println!("{}", serde_json::to_string(&res_val)?);
            io::stdout().flush()?;
        }

        line.clear();
    }

    Ok(())
}

async fn handle_request(req: JsonRpcRequest, state: Arc<AgentState>) -> Option<JsonRpcResponse> {
    let id = req.id.unwrap_or(json!(null));

    let result = match req.method.as_str() {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "logpose-agent",
                "version": "0.1.0"
            }
        })),
        "notifications/initialized" => return None,
        "tools/list" => Some(json!({
            "tools": [
                {
                    "name": "list_services",
                    "description": "List all registered services in LogPose registry",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "discover_instances",
                    "description": "Discover healthy instances for a specific service",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "service_code": {
                                "type": "string",
                                "description": "The unique code of the service"
                            }
                        },
                        "required": ["service_code"]
                    }
                },
                {
                    "name": "get_mesh_status",
                    "description": "Get an overview of the entire LogPose service mesh status",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        })),
        "tools/call" => {
            let params = req.params.and_then(|p| p.as_object().cloned()).unwrap_or_default();
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let tool_args = params.get("arguments").and_then(|v| v.as_object()).cloned().unwrap_or_default();

            match tool_name {
                "list_services" => {
                    match call_api(&state, "get", "/api/services").await {
                        Ok(data) => Some(json!({ "content": [{ "type": "text", "text": format!("Services: {}", data) }] })),
                        Err(e) => Some(json!({ "content": [{ "type": "text", "text": format!("Error: {}", e) }], "isError": true })),
                    }
                },
                "discover_instances" => {
                    let code = tool_args.get("service_code").and_then(|v| v.as_str()).unwrap_or_default();
                    match call_api(&state, "get", &format!("/api/discover/{}", code)).await {
                        Ok(data) => Some(json!({ "content": [{ "type": "text", "text": format!("Instances for {}: {}", code, data) }] })),
                        Err(e) => Some(json!({ "content": [{ "type": "text", "text": format!("Error: {}", e) }], "isError": true })),
                    }
                },
                "get_mesh_status" => {
                    match call_api(&state, "get", "/health").await {
                        Ok(data) => Some(json!({ "content": [{ "type": "text", "text": format!("Mesh Status: Server is {}", data) }] })),
                        Err(e) => Some(json!({ "content": [{ "type": "text", "text": format!("Error: {}", e) }], "isError": true })),
                    }
                },
                _ => Some(json!({ "content": [{ "type": "text", "text": "Tool not found" }], "isError": true })),
            }
        }
        _ => None,
    };

    result.map(|res| JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(res),
        error: None,
    })
}

async fn call_api(state: &AgentState, method: &str, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{}{}", state.server_url, path);
    let builder = match method {
        "get" => state.client.get(&url),
        "post" => state.client.post(&url),
        _ => return Err("Unsupported method".into()),
    };

    let mut builder = builder;
    if let Some(ref token) = state.token {
        builder = builder.bearer_auth(token);
    }

    let res = builder.send().await?;
    if res.status().is_success() {
        Ok(res.text().await?)
    } else {
        Err(format!("API Error: {}", res.status()).into())
    }
}
