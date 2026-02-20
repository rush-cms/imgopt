# Usage Guide

How to run, test, and deploy imgopt.

---

## Prerequisites

| Tool | Required for | Install |
|------|-------------|---------|
| Rust (stable) | Local development | [rustup.rs](https://rustup.rs) |
| `nasm` | Compiling the AV1 encoder (`rav1e`) | `apt install nasm` / `brew install nasm` |
| Docker | Container builds | [docs.docker.com](https://docs.docker.com/get-docker/) |

---

## Running locally

### 1. Configure environment variables

```bash
cp .env.example .env
```

Edit `.env` with your values:

```env
PORT=3000
API_TOKEN=your_secret_token_here
MAX_UPLOAD_MB=10
RUST_LOG=info
```

> `API_TOKEN` is required. The server refuses to start without it.

### 2. Start the server

```bash
cargo run
```

The server will be available at `http://localhost:3000`.

To see structured JSON logs while developing:

```bash
RUST_LOG=debug cargo run
```

---

## Running tests

```bash
API_TOKEN=any_value cargo test -- --test-threads=1
```

`--test-threads=1` is required because integration tests share the process environment.

To run only unit tests (faster):

```bash
API_TOKEN=any_value cargo test --lib
```

To run only integration tests:

```bash
API_TOKEN=any_value cargo test --test integration_tests -- --test-threads=1
```

---

## Running with Docker Compose

Useful for simulating the production environment locally.

```bash
# Build and start
docker compose up --build

# Run in background
docker compose up --build -d

# View logs
docker compose logs -f imgopt

# Stop
docker compose down
```

The service will be available at `http://localhost:3000`.

---

## Environment variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `API_TOKEN` | **yes** | — | Bearer token for authentication. The server exits on startup if missing or empty. |
| `PORT` | no | `3000` | TCP port the server listens on. |
| `MAX_UPLOAD_MB` | no | `10` | Maximum accepted upload size in megabytes. |
| `RUST_LOG` | no | `info` | Log verbosity. Accepts `error`, `warn`, `info`, `debug`, `trace`. |

---

## Deploying to Coolify

imgopt is designed to run as an internal service on the same Docker network as Laravel, without public exposure.

### 1. Push the repository

```bash
git push origin main
```

### 2. Create the application in Coolify

1. **New Resource → Application**
2. Select your Git repository
3. **Build Pack**: `Dockerfile` (auto-detected)
4. **Published Port**: `3000`

### 3. Set environment variables

In the **Environment Variables** tab, add:

```
API_TOKEN=a_long_random_secure_token
PORT=3000
MAX_UPLOAD_MB=10
RUST_LOG=info
```

Generate a strong token with:
```bash
openssl rand -hex 32
```

### 4. Configure health checks

In the **Health Check** tab:

| Field | Value |
|-------|-------|
| Path | `/health` |
| Port | `3000` |
| Interval | `30s` |
| Timeout | `5s` |

For the **readiness probe** (prevents traffic before the service is ready):

| Field | Value |
|-------|-------|
| Path | `/ready` |
| Port | `3000` |

### 5. Internal networking

Add imgopt to the same Coolify network as your Laravel application. Once on the same network, Laravel can reach imgopt using the service name as the hostname — no public port exposure needed:

```
http://imgopt:3000/convert
```

In your Laravel `.env`:

```env
IMGOPT_URL=http://imgopt:3000
IMGOPT_TOKEN=the_same_token_you_set_above
```

### 6. Automatic deploys

To trigger a redeploy on every push to `main`, enable the **Webhook** in Coolify's settings and add the provided URL to your repository's webhook configuration.

---

## Probe endpoints

| Endpoint | Purpose | Auth required |
|----------|---------|--------------|
| `GET /health` | Liveness — returns uptime and version | No |
| `GET /ready` | Readiness — confirms the service is accepting requests | No |

Both endpoints are intentionally excluded from authentication so orchestrators can poll them freely.
