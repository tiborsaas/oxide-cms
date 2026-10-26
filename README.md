# Oxide CMS 🦀

A lightweight, barebones headless Content Management System (CMS) written in Rust using **Axum** and **SQLite** (via **SQLx**).

Features:
- **SQLite Database**: Auto-initializes schema, runs migrations, enables WAL mode & foreign keys.
- **Minimalist Tera Blog**: Server-rendered blog templates using the Tera engine with frame/base layout, homepage listing, post reader, category filtering, and responsive styling.
- **Static Assets Serving**: Dedicated `/static` folder serving CSS, images, and fonts via `tower-http` with automatic MIME detection.
- **Single Source of Truth**: Unified service layer (`src/services/`) shared directly between the REST API and Tera templates to eliminate code duplication and maintain consistency.
- **Full RESTful JSON API**: Complete CRUD for posts and categories with slug routing and search under `/api`.
- **Posts & Categories Tables**: Rich relational modeling with timestamps, slugs, and publication states.
- **Admin Authentication**: Credentials configured securely via environment variables (`ADMIN_USERNAME` and `ADMIN_PASSWORD`), supporting both **JWT Bearer Tokens** and **HTTP Basic Authentication**.
- **Headless-Friendly**: Permissive CORS enabled for modern frontends (Next.js, Remix, Astro, Svelte, Vue), with public read endpoints for published content and admin guards for drafts and mutations.

---

## Getting Started

### 1. Prerequisites
- Rust (edition 2024 / 1.85+)
- Cargo

### 2. Environment Configuration
Copy `.env.example` to `.env`:
```bash
cp .env.example .env
```

Default configuration in `.env`:
```env
# Server
HOST=127.0.0.1
PORT=3000

# SQLite Database (local development)
DATABASE_URL=sqlite:data/oxide_cms.db

# Docker Persistent Storage Volume
# - Local dev: ./data (mounts host ./data for local SQL client inspection)
# - Production server: oxide_cms_data (Docker named volume on server disk)
DATA_VOLUME=./data

# Admin Credentials
ADMIN_USERNAME=admin
ADMIN_PASSWORD=admin_secret_pass

# JWT Secret
JWT_SECRET=super_secret_headless_cms_jwt_key_change_in_prod
```

### 3. Run the Server
```bash
cargo run
```
The server will start at `http://127.0.0.1:3000`.

---

## Minimalist Blog Web Interface (Tera)

Oxide CMS includes a built-in server-rendered frontend powered by the [Tera template engine](https://github.com/Keats/tera):

- **Homepage (`/`)**: Displays the latest published articles, category filter tabs, publication dates, and excerpts.
- **Category Filter (`/category/:slug`)**: Filters the post stream by selected category.
- **Article Reader (`/posts/:slug`)**: Full article view with metadata, tags, and formatted body.
- **Static Assets (`/static/*`)**: Serves public assets including `style.css` from the `static/` directory.

### Architecture: Single Source of Truth
Both the **REST API** (`/api/*`) and the **Server-Rendered Blog** (`/*`) share the exact same underlying service layer in [`src/services/`](src/services):
```
Client Request
      │
      ├──> /api/posts ─────────┐
      │                        │
      └──> / (HTML view) ──────┼──> src/services/posts::list_posts ──> SQLite DB
                               │
```
- No duplicate database queries or query-builder logic.
- Model modifications automatically benefit both JSON consumers and HTML views.

---

## Authentication

Admin mutation operations (`POST`, `PUT`, `DELETE`) and access to draft posts require authentication. You can authenticate using **either**:

1. **Bearer Token (JWT)**: Obtain a token from `/api/auth/login` and send `Authorization: Bearer <token>`.
2. **HTTP Basic Auth**: Pass admin username and password directly via `Authorization: Basic <base64(user:pass)>` (or `-u admin:admin_secret_pass` with `curl`).

---

## REST API Reference

### Health & Auth

#### Check API Health
```bash
curl http://127.0.0.1:3000/api/health
```

#### Admin Login (Obtain JWT)
```bash
curl -X POST http://127.0.0.1:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin_secret_pass"}'
```
Response:
```json
{
  "token": "eyJhbGciOi...",
  "token_type": "Bearer",
  "expires_in": 86400
}
```

#### Verify Authentication Status
```bash
curl http://127.0.0.1:3000/api/auth/me \
  -H "Authorization: Bearer <TOKEN>"
```

---

### Categories

| Method | Endpoint | Access | Description |
|---|---|---|---|
| `GET` | `/api/categories` | Public | List all categories |
| `GET` | `/api/categories/:id_or_slug` | Public | Get single category by ID or slug |
| `POST` | `/api/categories` | Admin | Create a category |
| `PUT` | `/api/categories/:id` | Admin | Update a category |
| `DELETE` | `/api/categories/:id` | Admin | Delete a category |

#### Create Category
```bash
curl -X POST http://127.0.0.1:3000/api/categories \
  -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Rust Tutorials",
    "description": "Guides and tutorials for modern Rust"
  }'
```

#### List Categories
```bash
curl http://127.0.0.1:3000/api/categories
```

#### Get Category by Slug
```bash
curl http://127.0.0.1:3000/api/categories/rust-tutorials
```

#### Update Category
```bash
curl -X PUT http://127.0.0.1:3000/api/categories/1 \
  -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Rust Guides"
  }'
```

#### Delete Category
```bash
curl -X DELETE http://127.0.0.1:3000/api/categories/1 \
  -H "Authorization: Bearer <TOKEN>"
```

---

### Posts

| Method | Endpoint | Access | Description |
|---|---|---|---|
| `GET` | `/api/posts` | Public (Admin sees drafts) | List posts with query filters |
| `GET` | `/api/posts/:id_or_slug` | Public (Admin sees drafts) | Get post by ID or slug with category |
| `POST` | `/api/posts` | Admin | Create a post |
| `PUT` | `/api/posts/:id` | Admin | Update a post |
| `DELETE` | `/api/posts/:id` | Admin | Delete a post |

#### Create Post
```bash
curl -X POST http://127.0.0.1:3000/api/posts \
  -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Building a Headless CMS in Rust",
    "content": "Rust and Axum make writing lightning-fast APIs a breeze...",
    "excerpt": "A short guide to building APIs in Rust",
    "published": true,
    "category_id": 1
  }'
```

#### List Posts (Query Filters)
Query parameters:
- `category_id`: filter by numeric category ID.
- `category_slug`: filter by category slug (e.g. `?category_slug=rust-tutorials`).
- `published`: `true` or `false` (only available to authenticated admins; guests always see published posts).
- `search`: search substring in title or content (e.g. `?search=Axum`).
- `limit`: number of results (default `20`, max `100`).
- `offset`: pagination offset (default `0`).

```bash
curl "http://127.0.0.1:3000/api/posts?category_slug=rust-tutorials&search=Axum"
```

#### Get Post by ID or Slug
```bash
curl http://127.0.0.1:3000/api/posts/building-a-headless-cms-in-rust
```
Response includes attached category details:
```json
{
  "id": 1,
  "title": "Building a Headless CMS in Rust",
  "slug": "building-a-headless-cms-in-rust",
  "content": "Rust and Axum make writing lightning-fast APIs a breeze...",
  "excerpt": "A short guide to building APIs in Rust",
  "published": true,
  "category": {
    "id": 1,
    "name": "Rust Tutorials",
    "slug": "rust-tutorials",
    "description": "Guides and tutorials for modern Rust",
    "created_at": "2026-10-12 12:00:00",
    "updated_at": "2026-10-12 12:00:00"
  },
  "created_at": "2026-10-12 12:05:00",
  "updated_at": "2026-10-12 12:05:00"
}
```

#### Update Post
```bash
curl -X PUT http://127.0.0.1:3000/api/posts/1 \
  -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Updated Title",
    "published": true
  }'
```

#### Delete Post
```bash
curl -X DELETE http://127.0.0.1:3000/api/posts/1 \
  -H "Authorization: Bearer <TOKEN>"
```

---

## Testing

Run automated integration test suite:
```bash
cargo test
```
The test suite validates:
- Health check
- Authentication (wrong password rejection, login token generation, basic auth, bearer auth)
- Category creation, slug uniqueness constraint, slug fetching, updates, deletions
- Post creation, category association, draft vs. published access control, searches, updates, and deletions

---

## Docker & Online Deployment

### 1. Quickstart with Docker Compose

To run Oxide CMS in Docker with persistent storage:

```bash
docker compose up -d --build
```

The container will build from the multi-stage `Dockerfile` and start the server on port `3000`.

To view logs:
```bash
docker compose logs -f oxide-cms
```

To stop the container:
```bash
docker compose down
```

---

### 2. SQLite Database Volume Guide

#### Why the Volume is Essential
SQLite is an embedded, file-based database. In containerized environments, containers are ephemeral—if you destroy or recreate a container without a mounted volume, **all database records will be lost**.

Oxide CMS stores its SQLite database inside the container at `/data/oxide_cms.db`. Because SQLite runs with **WAL mode (Write-Ahead Logging)** enabled for high performance, you will find three files inside `/data`:
1. `oxide_cms.db`: Main SQLite database file.
2. `oxide_cms.db-wal`: Write-Ahead Log storing uncommitted and recent transactions.
3. `oxide_cms.db-shm`: Shared-memory index for concurrent readers and writers.

> [!IMPORTANT]
> The database volume **must** preserve all three files (`.db`, `.db-wal`, and `.db-shm`) together. Never delete or detach the volume while transactions are in progress.

---

#### Inspecting SQLite Database Locally

Because the local database is kept at `./data/oxide_cms.db`, you can directly inspect, query, and modify it using your preferred SQL client app:
- **TablePlus / DBeaver / Beekeeper Studio / DB Browser for SQLite**: Open a new SQLite connection and select the file `./data/oxide_cms.db` in this repository.
- **SQLite CLI**:
  ```bash
  sqlite3 data/oxide_cms.db
  ```
Because SQLite uses **WAL mode**, keep in mind that active transactions write to `data/oxide_cms.db-wal` and `data/oxide_cms.db-shm` in the same directory. Modern SQL clients automatically handle WAL files. Both native execution (`cargo run`) and local Docker Compose (`docker compose up`) write directly to this same file.

---

#### Volume Configuration: Local vs Production Deployment

`docker-compose.yml` uses a configurable persistent volume `${DATA_VOLUME:-./data}:/data`:

```yaml
services:
  oxide-cms:
    volumes:
      - "${DATA_VOLUME:-./data}:/data"

volumes:
  oxide_cms_data:
    name: ${DATA_VOLUME_NAME:-oxide_cms_data}
```

##### 1. Local Development (Default: `./data`)
When `DATA_VOLUME` is omitted or set to `./data`, Docker Compose binds your host `./data` directory to `/data` in the container.
- Any changes made by the container are instantly reflected on your local disk at `./data/oxide_cms.db`.
- Easily inspectable via GUI SQL clients.

##### 2. Production Deployment (Docker Named Volume: `oxide_cms_data`)
On a production server, you want a persistent volume mounted from the server disk so that data is **not wiped when a new version of the app is deployed**:
- Set `DATA_VOLUME=oxide_cms_data` in your server's `.env` or PaaS settings (Dokploy, Coolify, Portainer).
- Docker stores and manages the data in `/var/lib/docker/volumes/oxide_cms_data/_data` on the server disk.
- When you deploy a new version, pull a new image, or rebuild the container, the new container mounts the exact same `oxide_cms_data` volume. No data is lost.

---

#### Backing Up and Restoring the Database

##### Backing Up (Hot Backup)
To safely backup the SQLite database without stopping the container or corrupting WAL files:

```bash
# Execute safe SQLite online backup directly from the container
docker compose exec oxide-cms sh -c "sqlite3 /data/oxide_cms.db '.backup /data/backup.db'"

# Copy the backup file to your host machine
docker compose cp oxide-cms:/data/backup.db ./oxide_cms_backup_$(date +%Y%m%d).db

# Remove the temporary backup file from container
docker compose exec oxide-cms rm /data/backup.db
```

##### Restoring a Backup
```bash
# 1. Stop the CMS container
docker compose stop oxide-cms

# 2. Copy the backup file into the volume
docker compose cp ./my_backup.db oxide-cms:/data/oxide_cms.db

# 3. Restart the container
docker compose start oxide-cms
```

---

### 3. Deploying Online

#### Deploying on a Cloud VPS (Hetzner, DigitalOcean, AWS, Linode)
1. Clone the repository on your server:
   ```bash
   git clone <your-repo-url> /opt/oxide-cms
   cd /opt/oxide-cms
   ```
2. Copy and configure production credentials in `.env`:
   ```bash
   cp .env.example .env
   # Edit with strong passwords:
   # ADMIN_PASSWORD=your_strong_random_password
   # JWT_SECRET=random_64_character_hex_string
   ```
3. Start the container in detached mode:
   ```bash
   docker compose up -d --build
   ```

#### Setting up HTTPS Reverse Proxy (Caddy Example)
Expose port 3000 securely with automated SSL using Caddy:
```caddyfile
cms.yourdomain.com {
    reverse_proxy localhost:3000
}
```

#### Deploying with Dokploy
Dokploy uses Traefik as its reverse proxy and runs its own dashboard on host port `3000`.

1. Create a new service in Dokploy and select **Compose** (or **Application** pointing to this Git repo).
2. **Ports & Routing**:
   - In Dokploy, go to **Domains** -> Add your domain (e.g., `cms.yourdomain.com`).
   - Set the target container port to **`3000`** (internal port).
   - Dokploy's Traefik will automatically route web traffic to container port 3000 internally.
   - Host port mapping in `docker-compose.yml` defaults to **`3001:3000`** (`HOST_PORT=3001`) to avoid colliding with the Dokploy dashboard on port 3000.
3. **Environment Variables**:
   - Set `ADMIN_USERNAME`, `ADMIN_PASSWORD`, and `JWT_SECRET` in Dokploy's Environment tab.
   - Set `DATA_VOLUME=oxide_cms_data` to mount the persistent Docker named volume from the server disk.
4. **Persistent SQLite Storage**:
   - The named volume `oxide_cms_data` persists across every new deployment, ensuring data is never wiped when Dokploy rebuilds or pulls a new version.

#### Deploying with Coolify / Portainer
1. Add a new service using **Docker Compose**.
2. Point it to this repository.
3. In the environment variables, set `DATA_VOLUME=oxide_cms_data` (along with `ADMIN_USERNAME`, `ADMIN_PASSWORD`, `JWT_SECRET`).
4. Docker will automatically mount the persistent named volume from the server disk to `/data`.

---

## Performance Evaluation (K6)

A complete load test suite is included in [`k6/performance_eval.js`](k6/performance_eval.js) to evaluate Oxide CMS under high concurrency.

The test simulates realistic traffic distributions:
- **30%**: Server-rendered HTML homepage (`/`) with Tera templates & SQLite reads.
- **20%**: Server-rendered single article reader (`/posts/:slug`).
- **25%**: REST API listings with filters (`/api/posts?limit=10` and `?search=...`).
- **15%**: Static asset serving (`/static/css/style.css`).
- **10%**: Authenticated admin write mutations (`POST /api/posts`).

### Running the Evaluation
```bash
# Run the multi-stage load test (up to 100 concurrent VUs)
k6 run k6/performance_eval.js

# Or run a quick custom stress test
k6 run --vus 50 --duration 15s k6/performance_eval.js
```

### Benchmark Results (Running in Docker on Apple Silicon)
- **Throughput**: ~**1,463 requests/second** (51,290 requests across 35s)
- **Error Rate**: **0.00%** (0 errors across 118,232 assertions)
- **Latency**:
  - Global `http_req_duration`: **avg: 4.99ms** | **p(95): 22.65ms** | **p(99): 38.11ms**
  - Static CSS (`/static/css/style.css`): **avg: 1.27ms** | **p(95): 2.78ms**
  - Single Post HTML (`/posts/:slug`): **avg: 2.68ms** | **p(95): 11.65ms**
  - REST API Posts (`/api/posts`): **avg: 3.78ms** | **p(95): 14.57ms**
  - Homepage HTML (`/`): **avg: 8.11ms** | **p(95): 29.77ms**
  - Authenticated SQLite Writes (`POST /api/posts`): **avg: 8.54ms** | **p(95): 33.23ms**
