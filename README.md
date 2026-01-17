# LogPose

LogPose is a high-performance service discovery system for microservices running across any network topology—cloud, on-prem, hybrid, or edge. Written in Rust for speed, safety, and predictability, LogPose provides a single source of truth for service presence, health, and reachability.

## Project Structure

LogPose is organized as a modular Rust workspace, ensuring separation of concerns and production readiness.

| Crate | Purpose |
| :--- | :--- |
| `logpose-server` | The high-performance core registry server (Axum 0.6). |
| `logpose-command` | Local administrative CLI for direct registry management. |
| `logpose-core` | Shared domain models, traits, and common logic. |
| `logpose-db` | SQLite storage implementation for persistence. |
| `logpose-agent` | **[Coming Soon]** Intelligent agent for AI-native service discovery and MCP integration. |

---

## Roadmap & Vision

LogPose is currently in its early stages of development, but it is architected from day one to become a **full-fledged, production-grade service discovery system**.

### AI & MCP Integration
The future of infrastructure is agentic. `logpose-agent` will be the bridge between traditional service discovery and AI-native workflows:
- **Model Context Protocol (MCP)**: Providing AI models with a standardized way to discover and interact with your service mesh.
- **Intelligent Routing**: AI-driven health analysis and predictive traffic management.
- **Natural Language Discovery**: Query your infrastructure using simple English.

---

## Integration Guide

To participate in the LogPose ecosystem, a service needs to be both **discoverable** and capable of **discovering** others.

### 1. Becoming Discoverable

For a service to be discoverable, it must be registered with the LogPose registry.

#### Registration
Services can be registered manually using the `logpose-command` CLI or programmatically via the API.
- **CLI**: `logpose-command instance add --service my-svc --address 10.0.0.5:8080 --protocol Http`
- **Manual API**: `POST /api/services/{code}/instances` (Requires Bearer Token)

#### Health Checks
LogPose's background worker periodically pings all registered instances. To ensure your service is marked as `Healthy`:
- **TCP Check**: By default, LogPose attempts a TCP connection to the registered `address`. Ensure your firewall allows incoming traffic on that port from the LogPose server.
- **Reporting Health**: Services can also proactively report their health status via `POST /api/instances/{id}/health`.

---

### 2. Discovering Other Services

Services can query LogPose to find the location of their dependencies.

#### Authentication
Discovery requests require a JWT token. Your service should:
1. Obtain a token: `POST /api/auth/token` with your `common_name`.
2. Include it in headers: `Authorization: Bearer <token>`.

#### Discovery API
To find instances for a specific service:
```bash
GET /api/discover/{service_code}
```
**Response Format**:
```json
[
  {
    "id": "uuid-string",
    "service_name": "auth-svc",
    "address": "127.0.0.1:8080",
    "protocol": "Http",
    "health": "Healthy"
  },
  {
    "id": "another-uuid",
    "service_name": "auth-svc",
    "address": "127.0.0.1:8081",
    "protocol": "Http",
    "health": "Healthy"
  }
]
```

### 3. Handling Multiple Instances

LogPose is designed to manage pools of service instances for high availability and scaling.

- **Multiple Registrations**: You can register multiple instances under the same `service_code`. Each will have a unique identity and be tracked independently.
- **Client-Side Load Balancing**: The Discovery API returns a list of all active instances. It is the responsibility of the discovering service (the client) to perform load balancing (e.g., Round Robin, Random, or Least Connections) based on this list.
- **Individual Health Monitoring**: The LogPose Health Worker monitors each instance independently. If one instance goes down, its status is updated to `Unhealthy`, allowing discovery clients to filter it out.

---

## Configuration

LogPose supports configuration via environment variables. You can also create a `.env` file in the root directory or within each crate's directory for easier management.

### Supported Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `DATABASE_URL` | Path to the SQLite database file | `logpose.db` |
| `JWT_SECRET` | Secret key used for signing/verifying JWT tokens | `super-secret-key` |
| `LOGPOSE_TOKEN` | (CLI only) JWT token for administrative API actions | *(None)* |

### Set up .env
```bash
# Example .env file content
DATABASE_URL=logpose.db
JWT_SECRET=your-secure-random-secret
```

---

## Getting Started

### Run the Server
```bash
cargo run -p logpose-server
```

### Use the CLI
```bash
cargo run -p logpose-command -- --help
```
