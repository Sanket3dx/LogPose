# LogPose

LogPose is a high-performance service discovery system for microservices running across any network topology—cloud, on-prem, hybrid, or edge. Written in Rust for speed, safety, and predictability, LogPose provides a single source of truth for service presence, health, and reachability.

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
  }
]
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
