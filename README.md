# Oxide CMS

An embeddable, headless Content Management System library for Rust applications built on Axum and SQLite.

`oxide-cms` provides a complete headless content layer: structured posts and categories, JWT-authenticated management REST APIs, SQLite database schema & migrations, CommonMark markdown parsing, and high-level async query interfaces.

---

## Features

- **Embedded Library**: Designed to be integrated directly into any Axum server or Rust application without external CMS daemon overhead.
- **Async Data Loading Interfaces**: High-level Rust functions for listing articles (with limit & pagination offset), querying articles by slug or ID, querying by category, querying by tag, and managing categories/tags.
- **Mountable Axum REST API**: Complete JWT-protected admin API for category/post/tag CRUD, plus public read endpoints (`/api/posts`, `/api/categories`, `/api/tags`, `/api/health`).
- **Zero-Config SQLite**: Built-in automated table migrations on connection initialization.
- **CommonMark Markdown Engine**: Fast CommonMark parsing with tables, strikethrough, task lists, footnotes, and language fences via `pulldown-cmark`.

---

## Installation

Install [`oxide-cms`](https://crates.io/crates/oxide-cms) from crates.io:

```sh
cargo add oxide-cms
```

Or add it to your `Cargo.toml` manually:

```toml
[dependencies]
oxide-cms = "0.0.1"
tokio = { version = "1", features = ["full"] }
axum = "0.8"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
```

---

## Ideal Usage & Architecture

`oxide-cms` separates content storage and REST API management from presentation templates. Consuming applications (such as portals, news feeds, or web apps) embed `oxide-cms` as their data and API engine while keeping their own frontend rendering and styling.

```text
┌────────────────────────────────────────────────────────┐
│             Consumer App (e.g. Your site)              │
│                                                        │
│   Web Server & Router (Axum)                           │
│   ├── Templates (Tera / Cyberpunk UI)                  │
│   ├── Custom Routes (/, /article/:slug, /design)       │
│   └── /api/*  ───► Mounts oxide_cms::api_router()      │
│                                                        │
│   Data Layer                                           │
│   ├── oxide_cms::list_articles(&pool, limit, offset)   │
│   ├── oxide_cms::get_article_by_slug(&pool, slug)      │
│   ├── oxide_cms::list_articles_by_tag(&pool, tag_slug) │
│   └── oxide_cms::markdown_to_html(&content)            │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
              ┌───────────────────────────┐
              │ SQLite Database (sqlx)    │
              │  - posts                  │
              │  - categories             │
              │  - tags                   │
              │  - post_tags              │
              └───────────────────────────┘
```

---

## Quickstart Example

```rust
use axum::{routing::get, response::Html, Router};
use oxide_cms::{
    api_router, get_article_by_slug, init_db, list_articles,
    markdown_to_html, AppConfig, AppState,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize configuration and database pool
    let config = AppConfig::from_env();
    let pool = init_db(&config.database_url).await?;

    // 2. Prepare shared state
    let state = AppState {
        pool: pool.clone(),
        config,
    };

    // 3. Build consumer application with custom web routes and embedded CMS API
    let app = Router::new()
        // Embedded Oxide CMS REST API under /api
        .nest("/api", api_router())
        // Consumer web routes using data loading interfaces
        .route("/", get(move || async move {
            let posts = list_articles(&pool, Some(10), Some(0)).await.unwrap_or_default();
            format!("Loaded {} articles from Oxide CMS", posts.len())
        }))
        .with_state(state);

    // 4. Start HTTP server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

---

## Data Loading Interfaces

`oxide-cms` exports ergonomic async query interfaces that return typed structs (`Post`, `PostDetail`, `Category`):

### 1. List Articles with Pagination

```rust
use oxide_cms::list_articles;

// Fetch latest 10 articles starting at offset 0
let limit = Some(10);
let offset = Some(0);
let articles = list_articles(&pool, limit, offset).await?;
for article in articles {
    println!("[{}] {}", article.slug, article.title);
}
```

### 2. Get Article by Slug

```rust
use oxide_cms::{get_article_by_slug, markdown_to_html};

if let Some(post) = get_article_by_slug(&pool, "ai-interior-surveillance").await? {
    println!("Title: {}", post.title);
    let html = markdown_to_html(&post.content);
}
```

### 3. List Articles by Category

```rust
use oxide_cms::list_articles_by_category;

let posts = list_articles_by_category(&pool, "surveillance", Some(20), None).await?;
```

### 4. List Articles by Tag

```rust
use oxide_cms::list_articles_by_tag;

let posts = list_articles_by_tag(&pool, "rust", Some(20), None).await?;
```

### 5. Category Queries

```rust
use oxide_cms::{list_categories, get_category_by_slug};

let categories = list_categories(&pool).await?;
let category = get_category_by_slug(&pool, "surveillance").await?;
```

### 6. Tag Queries

```rust
use oxide_cms::{list_tags, get_tag_by_slug};

let tags = list_tags(&pool).await?;
let tag = get_tag_by_slug(&pool, "rust").await?;
```

---

## REST API Reference

When mounting `oxide_cms::api_router()`, the following endpoints are provided:

| Method   | Endpoint                | Auth Required | Description                                                                                        |
| :------- | :---------------------- | :------------ | :------------------------------------------------------------------------------------------------- |
| `GET`    | `/api/health`           | No            | Healthcheck (`{"status":"healthy","database":"connected"}`)                                        |
| `POST`   | `/api/auth/login`       | No            | Authenticate admin; returns JWT token                                                              |
| `GET`    | `/api/auth/me`          | Bearer JWT    | Returns current authenticated user claims                                                          |
| `GET`    | `/api/categories`       | No            | List all categories                                                                                |
| `POST`   | `/api/categories`       | Bearer JWT    | Create category (`name`, `slug`)                                                                   |
| `GET`    | `/api/categories/:id`   | No            | Retrieve category by ID or slug                                                                    |
| `PUT`    | `/api/categories/:id`   | Bearer JWT    | Update category                                                                                    |
| `DELETE` | `/api/categories/:id`   | Bearer JWT    | Delete category                                                                                    |
| `GET`    | `/api/tags`             | No            | List all tags (supports `?search=`)                                                                |
| `POST`   | `/api/tags`             | Bearer JWT    | Create tag (`name`, `slug`)                                                                        |
| `GET`    | `/api/tags/:id_or_slug` | No            | Retrieve tag by ID or slug                                                                         |
| `PUT`    | `/api/tags/:id`         | Bearer JWT    | Update tag                                                                                         |
| `DELETE` | `/api/tags/:id`         | Bearer JWT    | Delete tag                                                                                         |
| `GET`    | `/api/posts`            | No            | List published posts (supports `?limit=`, `?offset=`, `?category_slug=`, `?tag_slug=`, `?search=`) |
| `POST`   | `/api/posts`            | Bearer JWT    | Create post (`title`, `slug`, `content`, `excerpt`, `category_id`, `tag_ids`, `published`)         |
| `GET`    | `/api/posts/:id`        | No            | Retrieve post by ID or slug                                                                        |
| `PUT`    | `/api/posts/:id`        | Bearer JWT    | Update post (`title`, `slug`, `content`, `excerpt`, `category_id`, `tag_ids`, `published`)         |
| `DELETE` | `/api/posts/:id`        | Bearer JWT    | Delete post                                                                                        |

---

## Configuration Reference

Configuration is loaded from environment variables or `.env` files via `AppConfig::from_env()`:

| Variable         | Default                             | Description                                 |
| :--------------- | :---------------------------------- | :------------------------------------------ |
| `DATABASE_URL`   | `sqlite:oxide_cms.db`               | SQLite connection string                    |
| `HOST`           | `127.0.0.1`                         | Default binding host                        |
| `PORT`           | `3000`                              | Default binding port                        |
| `ADMIN_USERNAME` | `admin`                             | Initial admin username                      |
| `ADMIN_PASSWORD` | `admin_secret_pass`                 | Initial admin password                      |
| `JWT_SECRET`     | `super_secret_headless_cms_jwt_key` | Secret key for signing and verifying tokens |

---

## Running Tests

```bash
cargo test
```
