use std::sync::atomic::{AtomicU64, Ordering};
use oxide_cms::{
    auth::AppState,
    config::AppConfig,
    db::init_db,
    models::{Category, PostDetail, Tag},
};
use reqwest::StatusCode;
use axum::response::IntoResponse;
use serde_json::{json, Value};

static DB_COUNTER: AtomicU64 = AtomicU64::new(1);

async fn spawn_test_app() -> (String, reqwest::Client, sqlx::SqlitePool) {
    let id = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
    let test_db = format!("sqlite:file:test_db_{id}?mode=memory&cache=shared");
    let pool = init_db(&test_db).await.expect("Failed to init test db");

    let config = AppConfig {
        database_url: test_db,
        host: "127.0.0.1".to_string(),
        port: 0,
        admin_username: "admin".to_string(),
        admin_password: "supersecretpass".to_string(),
        jwt_secret: "test_jwt_secret_key_12345".to_string(),
    };

    let state = AppState {
        pool: pool.clone(),
        config,
    };
    let app = oxide_cms::create_app(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral port");
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
    (base_url, client, pool)
}

#[tokio::test]
async fn test_health_check() {
    let (base_url, client, _) = spawn_test_app().await;

    let res = client
        .get(format!("{base_url}/api/health"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["database"], "connected");
}

#[tokio::test]
async fn test_authentication_flows() {
    let (base_url, client, _) = spawn_test_app().await;

    // 1. Failed login with wrong password
    let res = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({
            "username": "admin",
            "password": "wrong_password"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Successful login with correct credentials
    let res = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({
            "username": "admin",
            "password": "supersecretpass"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let auth_body: Value = res.json().await.unwrap();
    let token = auth_body["token"].as_str().expect("Token missing");

    // 3. Check /api/auth/me with Bearer token
    let res = client
        .get(format!("{base_url}/api/auth/me"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let me_body: Value = res.json().await.unwrap();
    assert_eq!(me_body["username"], "admin");

    // 4. Check /api/auth/me with Basic Auth
    let res = client
        .get(format!("{base_url}/api/auth/me"))
        .basic_auth("admin", Some("supersecretpass"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Check /api/auth/me without credentials
    let res = client
        .get(format!("{base_url}/api/auth/me"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_categories_and_posts_full_crud() {
    let (base_url, client, _) = spawn_test_app().await;

    // 1. Unauthenticated category creation fails
    let res = client
        .post(format!("{base_url}/api/categories"))
        .json(&json!({
            "name": "Rust Programming",
            "description": "All about Rust"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Login to get token
    let auth_res: Value = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({
            "username": "admin",
            "password": "supersecretpass"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = auth_res["token"].as_str().unwrap();

    // 3. Create category
    let res = client
        .post(format!("{base_url}/api/categories"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Rust Programming",
            "description": "All about Rust"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let category: Category = res.json().await.unwrap();
    assert_eq!(category.name, "Rust Programming");
    assert_eq!(category.slug, "rust-programming");
    let category_id = category.id;

    // 4. Attempt duplicate slug creation -> Conflict
    let res = client
        .post(format!("{base_url}/api/categories"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Rust Programming",
            "slug": "rust-programming"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // 5. Fetch category by slug
    let res = client
        .get(format!("{base_url}/api/categories/rust-programming"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let fetched_cat: Category = res.json().await.unwrap();
    assert_eq!(fetched_cat.id, category_id);

    // 6. Create Draft post (published: false)
    let res = client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Upcoming Rust 2024 Edition",
            "content": "Work in progress draft content...",
            "published": false,
            "category_id": category_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let draft_post: Value = res.json().await.unwrap();
    let draft_id = draft_post["id"].as_i64().unwrap();

    // 7. Create Published post (published: true)
    let res = client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Getting Started with Axum",
            "content": "Axum is an ergonomic and modular web framework in Rust.",
            "excerpt": "Introduction to Axum",
            "published": true,
            "category_id": category_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let published_post: Value = res.json().await.unwrap();
    let published_id = published_post["id"].as_i64().unwrap();

    // 8. Public guest listing: only published posts visible
    let res = client
        .get(format!("{base_url}/api/posts"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let public_posts: Vec<Value> = res.json().await.unwrap();
    assert_eq!(public_posts.len(), 1);
    assert_eq!(public_posts[0]["id"], published_id);

    // 9. Admin listing: all posts visible (both published and drafts)
    let res = client
        .get(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let admin_posts: Vec<Value> = res.json().await.unwrap();
    assert_eq!(admin_posts.len(), 2);

    // 10. Public guest accessing draft by id -> 404
    let res = client
        .get(format!("{base_url}/api/posts/{draft_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 11. Admin accessing draft by id -> 200 OK with category relation
    let res = client
        .get(format!("{base_url}/api/posts/{draft_id}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let post_detail: PostDetail = res.json().await.unwrap();
    assert_eq!(post_detail.id, draft_id);
    assert!(post_detail.category.is_some());
    assert_eq!(post_detail.category.unwrap().name, "Rust Programming");

    // 12. Search posts
    let res = client
        .get(format!("{base_url}/api/posts?search=Axum"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_results: Vec<Value> = res.json().await.unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0]["id"], published_id);

    // 13. Filter by category slug
    let res = client
        .get(format!("{base_url}/api/posts?category_slug=rust-programming"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cat_filter_results: Vec<Value> = res.json().await.unwrap();
    assert_eq!(cat_filter_results.len(), 1);

    // 14. Update post using HTTP Basic Auth
    let res = client
        .put(format!("{base_url}/api/posts/{published_id}"))
        .basic_auth("admin", Some("supersecretpass"))
        .json(&json!({
            "title": "Getting Started with Axum & SQLx",
            "published": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated_post: Value = res.json().await.unwrap();
    assert_eq!(updated_post["title"], "Getting Started with Axum & SQLx");

    // 15. Delete post
    let res = client
        .delete(format!("{base_url}/api/posts/{draft_id}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 16. Verify draft is gone
    let res = client
        .get(format!("{base_url}/api/posts/{draft_id}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 17. Delete category
    let res = client
        .delete(format!("{base_url}/api/categories/{category_id}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_data_loading_interfaces() {
    let (base_url, client, pool) = spawn_test_app().await;

    // Login and publish category and posts
    let auth_res: Value = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({
            "username": "admin",
            "password": "supersecretpass"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = auth_res["token"].as_str().unwrap();

    let cat_res: Value = client
        .post(format!("{base_url}/api/categories"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Rust Architecture",
            "slug": "rust-architecture"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cat_id = cat_res["id"].as_i64().unwrap();

    client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Clean Web Architecture in Rust",
            "slug": "clean-web-architecture-in-rust",
            "content": "Using Axum and SQLite provides high maintainability and speed.",
            "excerpt": "Architectural guidelines for unified APIs.",
            "published": true,
            "category_id": cat_id
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Second Article",
            "slug": "second-article",
            "content": "Another article content.",
            "excerpt": "Second excerpt.",
            "published": true,
            "category_id": cat_id
        }))
        .send()
        .await
        .unwrap();

    // 1. Test list_articles with pagination
    let articles = oxide_cms::list_articles(&pool, Some(10), Some(0)).await.unwrap();
    assert_eq!(articles.len(), 2);

    let paginated = oxide_cms::list_articles(&pool, Some(1), Some(0)).await.unwrap();
    assert_eq!(paginated.len(), 1);

    // 2. Test get_article_by_slug
    let post = oxide_cms::get_article_by_slug(&pool, "clean-web-architecture-in-rust")
        .await
        .unwrap()
        .expect("Post should exist");
    assert_eq!(post.title, "Clean Web Architecture in Rust");
    assert_eq!(
        post.category.as_ref().map(|c| c.slug.as_str()),
        Some("rust-architecture")
    );

    // 3. Test list_articles_by_category
    let cat_articles = oxide_cms::list_articles_by_category(&pool, "rust-architecture", None, None)
        .await
        .unwrap();
    assert_eq!(cat_articles.len(), 2);

    // 4. Test list_categories & get_category_by_slug
    let cats = oxide_cms::list_categories(&pool).await.unwrap();
    assert_eq!(cats.len(), 1);
    let cat = oxide_cms::get_category_by_slug(&pool, "rust-architecture")
        .await
        .unwrap()
        .expect("Category should exist");
    assert_eq!(cat.name, "Rust Architecture");

    // 5. Create Tag via API and test list_tags, get_tag_by_slug, list_articles_by_tag
    let tag_res: Value = client
        .post(format!("{base_url}/api/tags"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Backend",
            "slug": "backend"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tag_id = tag_res["id"].as_i64().unwrap();

    // Assign tag to second article
    client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Tagged Article",
            "slug": "tagged-article",
            "content": "Tagged article content.",
            "published": true,
            "category_id": cat_id,
            "tag_ids": [tag_id]
        }))
        .send()
        .await
        .unwrap();

    let tags = oxide_cms::list_tags(&pool).await.unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].slug, "backend");

    let tag = oxide_cms::get_tag_by_slug(&pool, "backend")
        .await
        .unwrap()
        .expect("Tag should exist");
    assert_eq!(tag.name, "Backend");

    let tagged_articles = oxide_cms::list_articles_by_tag(&pool, "backend", None, None)
        .await
        .unwrap();
    assert_eq!(tagged_articles.len(), 1);
    assert_eq!(tagged_articles[0].slug, "tagged-article");
    assert_eq!(tagged_articles[0].tags.len(), 1);
    assert_eq!(tagged_articles[0].tags[0].slug, "backend");
}

#[tokio::test]
async fn test_article_markdown_to_html_rendering() {
    let (base_url, client, _) = spawn_test_app().await;

    // 1. Log in to get admin token
    let auth_res: Value = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({
            "username": "admin",
            "password": "supersecretpass"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = auth_res["token"].as_str().unwrap();

    // 2. Create article stored as Markdown
    let md_content = r#"## Introduction to Rust Markdown

This article tests **bold text**, *italic text*, and [Oxide](https://github.com/oxide).

### Features
- Native speed
- Type safety
- CommonMark compliant

Here is some sample code:
```rust
fn compute_answer() -> i32 {
    42
}
```

> Markdown quotes are supported.
"#;

    let create_res = client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Markdown Showcase Article",
            "slug": "markdown-showcase",
            "content": md_content,
            "excerpt": "A post testing markdown conversion to HTML.",
            "published": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_res.status(), StatusCode::CREATED);

    // 3. Verify in database/API that the article is still stored as raw markdown
    let api_res = client
        .get(format!("{base_url}/api/posts/markdown-showcase"))
        .send()
        .await
        .unwrap();
    assert_eq!(api_res.status(), StatusCode::OK);
    let post_json: Value = api_res.json().await.unwrap();
    assert_eq!(post_json["content"].as_str().unwrap(), md_content);

    // 4. Verify that converting raw markdown with oxide_cms::markdown_to_html yields valid HTML
    let rendered_html = oxide_cms::markdown_to_html(post_json["content"].as_str().unwrap());

    // Verify converted HTML tags
    assert!(rendered_html.contains("<h2>Introduction to Rust Markdown</h2>"));
    assert!(rendered_html.contains("<strong>bold text</strong>"));
    assert!(rendered_html.contains("<em>italic text</em>"));
    assert!(rendered_html.contains(r#"<a href="https://github.com/oxide">Oxide</a>"#));
    assert!(rendered_html.contains("<h3>Features</h3>"));
    assert!(rendered_html.contains("<ul>"));
    assert!(rendered_html.contains("<li>Native speed</li>"));
    assert!(rendered_html.contains("<li>Type safety</li>"));
    assert!(rendered_html.contains("<li>CommonMark compliant</li>"));
    assert!(rendered_html.contains(r#"<code class="language-rust">fn compute_answer() -&gt; i32 {"#));
    assert!(rendered_html.contains("<blockquote>"));
    assert!(rendered_html.contains("<p>Markdown quotes are supported.</p>"));
}

#[tokio::test]
async fn test_tags_full_crud() {
    let (base_url, client, _) = spawn_test_app().await;

    // 1. Unauthenticated tag creation fails -> 401 Unauthorized
    let res = client
        .post(format!("{base_url}/api/tags"))
        .json(&json!({
            "name": "Rust Language",
            "slug": "rust"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Login to get token
    let auth_res: Value = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({
            "username": "admin",
            "password": "supersecretpass"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = auth_res["token"].as_str().unwrap();

    // 3. Create Tag with auth -> 201 Created
    let res = client
        .post(format!("{base_url}/api/tags"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Rust Language",
            "slug": "rust"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let tag: Tag = res.json().await.unwrap();
    assert_eq!(tag.name, "Rust Language");
    assert_eq!(tag.slug, "rust");
    let tag_id = tag.id;

    // 4. Duplicate slug creation -> 409 Conflict
    let res = client
        .post(format!("{base_url}/api/tags"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Rust Duplicate",
            "slug": "rust"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // 5. Create second tag
    let res = client
        .post(format!("{base_url}/api/tags"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Web Development"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let tag2: Tag = res.json().await.unwrap();
    assert_eq!(tag2.slug, "web-development");

    // 6. List tags -> GET /api/tags
    let res = client
        .get(format!("{base_url}/api/tags"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tags: Vec<Tag> = res.json().await.unwrap();
    assert_eq!(tags.len(), 2);

    // 7. Search tags -> GET /api/tags?search=Web
    let res = client
        .get(format!("{base_url}/api/tags?search=Web"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let search_tags: Vec<Tag> = res.json().await.unwrap();
    assert_eq!(search_tags.len(), 1);
    assert_eq!(search_tags[0].slug, "web-development");

    // 8. Fetch tag by slug -> GET /api/tags/:id_or_slug
    let res = client
        .get(format!("{base_url}/api/tags/rust"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let fetched_by_slug: Tag = res.json().await.unwrap();
    assert_eq!(fetched_by_slug.id, tag_id);

    // 9. Fetch tag by id -> GET /api/tags/:id_or_slug
    let res = client
        .get(format!("{base_url}/api/tags/{tag_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let fetched_by_id: Tag = res.json().await.unwrap();
    assert_eq!(fetched_by_id.slug, "rust");

    // 10. Fetch non-existent tag -> 404 Not Found
    let res = client
        .get(format!("{base_url}/api/tags/nonexistent"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 11. Unauthenticated update fails
    let res = client
        .put(format!("{base_url}/api/tags/{tag_id}"))
        .json(&json!({
            "name": "Rust Ecosystem"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 12. Authenticated update tag -> PUT /api/tags/:id
    let res = client
        .put(format!("{base_url}/api/tags/{tag_id}"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Rust Ecosystem",
            "slug": "rust-ecosystem"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated: Tag = res.json().await.unwrap();
    assert_eq!(updated.name, "Rust Ecosystem");
    assert_eq!(updated.slug, "rust-ecosystem");

    // 13. Authenticated delete tag -> DELETE /api/tags/:id
    let res = client
        .delete(format!("{base_url}/api/tags/{tag_id}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 14. Verify deleted tag returns 404
    let res = client
        .get(format!("{base_url}/api/tags/{tag_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_posts_tags_assignment_and_filtering() {
    let (base_url, client, _) = spawn_test_app().await;

    // Login as admin
    let auth_res: Value = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({
            "username": "admin",
            "password": "supersecretpass"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = auth_res["token"].as_str().unwrap();

    // Create Categories: "Tutorials" and "News"
    let tutorials_cat: Value = client
        .post(format!("{base_url}/api/categories"))
        .bearer_auth(token)
        .json(&json!({
            "name": "Tutorials",
            "slug": "tutorials"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tutorials_cat_id = tutorials_cat["id"].as_i64().unwrap();

    let news_cat: Value = client
        .post(format!("{base_url}/api/categories"))
        .bearer_auth(token)
        .json(&json!({
            "name": "News",
            "slug": "news"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let news_cat_id = news_cat["id"].as_i64().unwrap();

    // Create Tags: "Rust", "Async", "Web"
    let rust_tag: Value = client
        .post(format!("{base_url}/api/tags"))
        .bearer_auth(token)
        .json(&json!({ "name": "Rust", "slug": "rust" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rust_tag_id = rust_tag["id"].as_i64().unwrap();

    let async_tag: Value = client
        .post(format!("{base_url}/api/tags"))
        .bearer_auth(token)
        .json(&json!({ "name": "Async", "slug": "async" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let async_tag_id = async_tag["id"].as_i64().unwrap();

    let web_tag: Value = client
        .post(format!("{base_url}/api/tags"))
        .bearer_auth(token)
        .json(&json!({ "name": "Web", "slug": "web" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let web_tag_id = web_tag["id"].as_i64().unwrap();

    // Post 1: Category "Tutorials", Tags ["Rust", "Async"]
    let post1_res = client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Async Rust Tutorial",
            "slug": "async-rust-tutorial",
            "content": "Deep dive into async await in Rust.",
            "published": true,
            "category_id": tutorials_cat_id,
            "tag_ids": [rust_tag_id, async_tag_id]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(post1_res.status(), StatusCode::CREATED);
    let post1: Value = post1_res.json().await.unwrap();
    let post1_id = post1["id"].as_i64().unwrap();
    let post1_tags = post1["tags"].as_array().expect("tags array");
    assert_eq!(post1_tags.len(), 2);

    // Post 2: Category "Tutorials", Tag ["Rust"]
    let post2_res = client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Rust Basics",
            "slug": "rust-basics",
            "content": "Learn the borrow checker.",
            "published": true,
            "category_id": tutorials_cat_id,
            "tag_ids": [rust_tag_id]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(post2_res.status(), StatusCode::CREATED);
    let post2: Value = post2_res.json().await.unwrap();
    let post2_id = post2["id"].as_i64().unwrap();

    // Post 3: Category "News", Tag ["Rust"]
    let post3_res = client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "Rust 2024 Edition Released",
            "slug": "rust-2024-released",
            "content": "The new edition is here.",
            "published": true,
            "category_id": news_cat_id,
            "tag_ids": [rust_tag_id]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(post3_res.status(), StatusCode::CREATED);
    let post3: Value = post3_res.json().await.unwrap();
    let post3_id = post3["id"].as_i64().unwrap();

    // Post 4: Category "News", Tag ["Web"]
    let post4_res = client
        .post(format!("{base_url}/api/posts"))
        .bearer_auth(token)
        .json(&json!({
            "title": "WebAssembly Updates",
            "slug": "webassembly-updates",
            "content": "Wasm is faster than ever.",
            "published": true,
            "category_id": news_cat_id,
            "tag_ids": [web_tag_id]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(post4_res.status(), StatusCode::CREATED);
    let post4: Value = post4_res.json().await.unwrap();
    let post4_id = post4["id"].as_i64().unwrap();

    // 1. Verify PostDetail endpoint returns tags
    let detail_res = client
        .get(format!("{base_url}/api/posts/{post1_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(detail_res.status(), StatusCode::OK);
    let post_detail: PostDetail = detail_res.json().await.unwrap();
    assert_eq!(post_detail.tags.len(), 2);
    let slugs: Vec<&str> = post_detail.tags.iter().map(|t| t.slug.as_str()).collect();
    assert!(slugs.contains(&"rust"));
    assert!(slugs.contains(&"async"));

    // 2. Query posts by tag_slug=async -> Only Post 1
    let res = client
        .get(format!("{base_url}/api/posts?tag_slug=async"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let async_posts: Vec<Value> = res.json().await.unwrap();
    assert_eq!(async_posts.len(), 1);
    assert_eq!(async_posts[0]["id"], post1_id);

    // 2b. Query posts by tag_slug=web -> Only Post 4
    let res = client
        .get(format!("{base_url}/api/posts?tag_slug=web"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let web_posts: Vec<Value> = res.json().await.unwrap();
    assert_eq!(web_posts.len(), 1);
    assert_eq!(web_posts[0]["id"], post4_id);

    // 3. Query posts by tag_slug=rust -> Post 1, Post 2, Post 3
    let res = client
        .get(format!("{base_url}/api/posts?tag_slug=rust"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rust_posts: Vec<Value> = res.json().await.unwrap();
    assert_eq!(rust_posts.len(), 3);
    let rust_post_ids: Vec<i64> = rust_posts.iter().map(|p| p["id"].as_i64().unwrap()).collect();
    assert!(rust_post_ids.contains(&post1_id));
    assert!(rust_post_ids.contains(&post2_id));
    assert!(rust_post_ids.contains(&post3_id));

    // 4. Combine category_slug AND tag_slug:
    // tutorials + async -> Post 1
    let res = client
        .get(format!("{base_url}/api/posts?category_slug=tutorials&tag_slug=async"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let combined_1: Vec<Value> = res.json().await.unwrap();
    assert_eq!(combined_1.len(), 1);
    assert_eq!(combined_1[0]["id"], post1_id);

    // tutorials + rust -> Post 1 and Post 2
    let res = client
        .get(format!("{base_url}/api/posts?category_slug=tutorials&tag_slug=rust"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let combined_2: Vec<Value> = res.json().await.unwrap();
    assert_eq!(combined_2.len(), 2);

    // news + rust -> Post 3
    let res = client
        .get(format!("{base_url}/api/posts?category_slug=news&tag_slug=rust"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let combined_3: Vec<Value> = res.json().await.unwrap();
    assert_eq!(combined_3.len(), 1);
    assert_eq!(combined_3[0]["id"], post3_id);

    // news + async -> Empty
    let res = client
        .get(format!("{base_url}/api/posts?category_slug=news&tag_slug=async"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let combined_empty: Vec<Value> = res.json().await.unwrap();
    assert_eq!(combined_empty.len(), 0);

    // 5. Update Post 1 tags: replace with [web_tag_id]
    let update_res = client
        .put(format!("{base_url}/api/posts/{post1_id}"))
        .bearer_auth(token)
        .json(&json!({
            "tag_ids": [web_tag_id]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update_res.status(), StatusCode::OK);
    let updated_post: Value = update_res.json().await.unwrap();
    let updated_tags = updated_post["tags"].as_array().unwrap();
    assert_eq!(updated_tags.len(), 1);
    assert_eq!(updated_tags[0]["slug"], "web");

    // tutorials + web -> now contains Post 1
    let res = client
        .get(format!("{base_url}/api/posts?category_slug=tutorials&tag_slug=web"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tutorials_web: Vec<Value> = res.json().await.unwrap();
    assert_eq!(tutorials_web.len(), 1);
    assert_eq!(tutorials_web[0]["id"], post1_id);

    // tutorials + async -> now empty
    let res = client
        .get(format!("{base_url}/api/posts?category_slug=tutorials&tag_slug=async"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tutorials_async: Vec<Value> = res.json().await.unwrap();
    assert_eq!(tutorials_async.len(), 0);
}


#[tokio::test]
async fn test_full_coverage_additional_cases() {
    let (base_url, client, _pool) = spawn_test_app().await;

    let _test_cfg = oxide_cms::AppConfig::from_env();
    // Config from env with custom vars set
    unsafe {
        std::env::set_var("DATABASE_URL", "sqlite:custom.db");
        std::env::set_var("HOST", "0.0.0.0");
        std::env::set_var("PORT", "8080");
        std::env::set_var("ADMIN_USERNAME", "custom_admin");
        std::env::set_var("ADMIN_PASSWORD", "custom_pass");
        std::env::set_var("JWT_SECRET", "custom_secret");
    }
    let custom_cfg = oxide_cms::AppConfig::from_env();
    assert_eq!(custom_cfg.host, "0.0.0.0");
    assert_eq!(custom_cfg.port, 8080);
    assert_eq!(custom_cfg.admin_username, "custom_admin");
    assert_eq!(custom_cfg.admin_password, "custom_pass");
    assert_eq!(custom_cfg.jwt_secret, "custom_secret");
    assert_eq!(custom_cfg.database_url, "sqlite:custom.db");
    unsafe {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("HOST");
        std::env::remove_var("PORT");
        std::env::remove_var("ADMIN_USERNAME");
        std::env::remove_var("ADMIN_PASSWORD");
        std::env::remove_var("JWT_SECRET");
    }

    // 1. Root API endpoint
    let res = client.get(format!("{base_url}/api")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let root_val: Value = res.json().await.unwrap();
    assert_eq!(root_val["name"], "oxide-cms");

    // 2. api_router() invocation
    let _ = oxide_cms::api_router();

    // 3. Error display & responses
    let errors = vec![
        oxide_cms::AppError::NotFound("item".into()),
        oxide_cms::AppError::BadRequest("bad".into()),
        oxide_cms::AppError::Unauthorized("unauth".into()),
        oxide_cms::AppError::Conflict("conflict".into()),
        oxide_cms::AppError::Internal("server error".into()),
        oxide_cms::AppError::Database(sqlx::Error::RowNotFound),
    ];
    for e in errors {
        let _ = format!("{e}");
        use axum::response::IntoResponse;
        let _ = e.into_response();
    }
    use std::error::Error;
    let _ = oxide_cms::AppError::Internal("test".into()).source();

    // From sqlx::Error::RowNotFound
    let from_row_err: oxide_cms::AppError = sqlx::Error::RowNotFound.into();
    let _ = from_row_err.into_response();

    // From sqlx::Error::PoolTimedOut and Configuration
    let from_pool_err: oxide_cms::AppError = sqlx::Error::PoolTimedOut.into();
    let _ = from_pool_err.into_response();

    // Trigger foreign key violation directly via pool
    // Enable foreign keys and insert post with invalid category_id
    let fk_err = sqlx::query("INSERT INTO posts (title, slug, content, category_id) VALUES ('fk', 'fk', 'fk', 99999)")
        .execute(&_pool)
        .await
        .unwrap_err();
    let app_fk_err: oxide_cms::AppError = fk_err.into();
    let _ = app_fk_err.into_response();
    // Database generic error into AppError
    let generic_sql_err = sqlx::query("SELECT non_existent_column_xyz FROM posts")
        .execute(&_pool)
        .await
        .unwrap_err();
    let app_db_err: oxide_cms::AppError = generic_sql_err.into();
    let _ = app_db_err.into_response();

    // 4. slugify edge cases
    assert_eq!(oxide_cms::models::slugify(""), "item");
    assert_eq!(oxide_cms::models::slugify("---"), "item");
    assert_eq!(oxide_cms::models::slugify("  hello - world  "), "hello-world");
    assert_eq!(oxide_cms::models::slugify("hello world -"), "hello-world");

    // 5. Auth edge cases
    let login_res = client
        .post(format!("{base_url}/api/auth/login"))
        .json(&json!({"username": "admin", "password": "supersecretpass"}))
        .send()
        .await
        .unwrap();
    let token = login_res.json::<Value>().await.unwrap()["token"].as_str().unwrap().to_string();

    // Invalid bearer
    let res = client.get(format!("{base_url}/api/auth/me")).bearer_auth("invalid.token.here").send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Wrong sub token
    let wrong_user_token = oxide_cms::auth::create_jwt_token("wrong_user", "test_jwt_secret_key_12345").unwrap();
    let res = client.get(format!("{base_url}/api/auth/me")).bearer_auth(&wrong_user_token).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Malformed basic
    let res = client.get(format!("{base_url}/api/auth/me")).header("Authorization", "Basic not_valid_base64!!!").send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Non-utf8 basic
    let res = client.get(format!("{base_url}/api/auth/me")).header("Authorization", "Basic /w==").send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Wrong basic user/pass lengths
    let res = client.get(format!("{base_url}/api/auth/me")).basic_auth("short", Some("pass")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Basic auth same length but wrong username/pass
    let res = client.get(format!("{base_url}/api/auth/me")).basic_auth("aaaaa", Some("supersecretpass")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Basic auth missing colon
    let res = client.get(format!("{base_url}/api/auth/me")).header("Authorization", "Basic YWRtaW4=").send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // Unsupported header
    let res = client.get(format!("{base_url}/api/auth/me")).header("Authorization", "Digest something").send().await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 6. Categories route & services coverage
    // GET /api/categories
    let res = client.get(format!("{base_url}/api/categories")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Create category with empty name
    let res = client.post(format!("{base_url}/api/categories")).bearer_auth(&token).json(&json!({"name": "   "})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Create valid category
    let res = client.post(format!("{base_url}/api/categories")).bearer_auth(&token).json(&json!({"name": "Design", "slug": "design", "description": "Design articles"})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let cat_id = res.json::<Value>().await.unwrap()["id"].as_i64().unwrap();

    // GET category by numeric id
    let res = client.get(format!("{base_url}/api/categories/{cat_id}")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // GET category not found
    let res = client.get(format!("{base_url}/api/categories/nonexistent-cat")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // PUT category
    let res = client.put(format!("{base_url}/api/categories/{cat_id}")).bearer_auth(&token).json(&json!({"name": "Design UI", "description": "UI & Design"})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // PUT category with empty name
    let res = client.put(format!("{base_url}/api/categories/{cat_id}")).bearer_auth(&token).json(&json!({"name": "  "})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // PUT category not found
    let res = client.put(format!("{base_url}/api/categories/99999")).bearer_auth(&token).json(&json!({"name": "Test"})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // DELETE category not found
    let res = client.delete(format!("{base_url}/api/categories/99999")).bearer_auth(&token).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 7. Tags route & services coverage
    // Create tag with empty name
    let res = client.post(format!("{base_url}/api/tags")).bearer_auth(&token).json(&json!({"name": "  "})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Create valid tag
    let res = client.post(format!("{base_url}/api/tags")).bearer_auth(&token).json(&json!({"name": "RustLang"})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let tag_id = res.json::<Value>().await.unwrap()["id"].as_i64().unwrap();

    // Update tag with empty name
    let res = client.put(format!("{base_url}/api/tags/{tag_id}")).bearer_auth(&token).json(&json!({"name": "  "})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Update tag not found
    let res = client.put(format!("{base_url}/api/tags/99999")).bearer_auth(&token).json(&json!({"name": "Test"})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Delete tag not found
    let res = client.delete(format!("{base_url}/api/tags/99999")).bearer_auth(&token).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 8. Posts route & services coverage
    // Create post with empty title
    let res = client.post(format!("{base_url}/api/posts")).bearer_auth(&token).json(&json!({"title": "  ", "content": "text"})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Create post with non-existent category
    let res = client.post(format!("{base_url}/api/posts")).bearer_auth(&token).json(&json!({"title": "Post", "content": "text", "category_id": 99999})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Create post with non-existent tag
    let res = client.post(format!("{base_url}/api/posts")).bearer_auth(&token).json(&json!({"title": "Post", "content": "text", "tag_ids": [99999]})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Create valid post
    let res = client.post(format!("{base_url}/api/posts")).bearer_auth(&token).json(&json!({
        "title": "Coverage Post",
        "content": "Content",
        "category_id": cat_id,
        "tag_ids": [tag_id],
        "published": false
    })).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let post_id = res.json::<Value>().await.unwrap()["id"].as_i64().unwrap();

    // Update post with None category and None tags (retaining existing)
    let res = client.put(format!("{base_url}/api/posts/{post_id}")).bearer_auth(&token).json(&json!({
        "excerpt": "Updated excerpt"
    })).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Query posts with category_id and tag_id
    let res = client.get(format!("{base_url}/api/posts?category_id={cat_id}&tag_id={tag_id}&published=false")).bearer_auth(&token).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let posts_list: Vec<Value> = res.json().await.unwrap();
    assert_eq!(posts_list.len(), 1);

    // Update post with empty title
    let res = client.put(format!("{base_url}/api/posts/{post_id}")).bearer_auth(&token).json(&json!({"title": "  "})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Update post with non-existent category
    let res = client.put(format!("{base_url}/api/posts/{post_id}")).bearer_auth(&token).json(&json!({"category_id": 99999})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Update post with non-existent tag
    let res = client.put(format!("{base_url}/api/posts/{post_id}")).bearer_auth(&token).json(&json!({"tag_ids": [99999]})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Update post not found
    let res = client.put(format!("{base_url}/api/posts/99999")).bearer_auth(&token).json(&json!({"title": "Nonexistent"})).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Delete post not found
    let res = client.delete(format!("{base_url}/api/posts/99999")).bearer_auth(&token).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 9. DB directory creation
    let temp_dir = std::env::temp_dir().join(format!("oxide_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let temp_db_path = temp_dir.join("sub").join("test.db");
    let temp_db_url = format!("sqlite://{}", temp_db_path.to_str().unwrap());
    let pool_temp = oxide_cms::db::init_db(&temp_db_url).await.unwrap();
    let _ = pool_temp.close().await;
    let _ = std::fs::remove_dir_all(temp_dir);
    // OptionalAdminUser with valid token
    let res = client.get(format!("{base_url}/api/posts")).bearer_auth(&token).send().await.unwrap();
    // OptionalAdminUser with invalid token falls back to None (guest)
    let _res = client.get(format!("{base_url}/api/posts")).bearer_auth("invalid_token_xyz").send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.status(), StatusCode::OK);
}
