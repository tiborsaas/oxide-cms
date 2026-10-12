use std::sync::atomic::{AtomicU64, Ordering};
use oxide_cms::{
    auth::AppState,
    config::AppConfig,
    db::init_db,
    models::{Category, PostDetail},
};
use reqwest::StatusCode;
use serde_json::{json, Value};

static DB_COUNTER: AtomicU64 = AtomicU64::new(1);

async fn spawn_test_app() -> (String, reqwest::Client) {
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

    let tera = tera::Tera::new("templates/**/*.html").expect("Failed to initialize Tera templates");
    let state = AppState {
        pool,
        config,
        tera: std::sync::Arc::new(tera),
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
    (base_url, client)
}

#[tokio::test]
async fn test_health_check() {
    let (base_url, client) = spawn_test_app().await;

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
    let (base_url, client) = spawn_test_app().await;

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
    let (base_url, client) = spawn_test_app().await;

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
async fn test_web_templates_and_static_assets() {
    let (base_url, client) = spawn_test_app().await;

    // 1. Verify static CSS stylesheet is served
    let css_res = client
        .get(format!("{base_url}/static/css/style.css"))
        .send()
        .await
        .unwrap();
    assert_eq!(css_res.status(), StatusCode::OK);
    let css_content = css_res.text().await.unwrap();
    assert!(css_content.contains("Oxide CMS - Minimalist Blog Stylesheet"));

    // 2. Verify homepage renders HTML
    let home_res = client
        .get(format!("{base_url}/"))
        .send()
        .await
        .unwrap();
    assert_eq!(home_res.status(), StatusCode::OK);
    let home_html = home_res.text().await.unwrap();
    assert!(home_html.contains("<!DOCTYPE html>"));
    assert!(home_html.contains("brand-name"));
    assert!(home_html.contains("Engineering & Design Notes"));

    // 3. Login and publish a post with a category
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
            "content": "Using Axum, SQLite, and Tera provides high maintainability and speed.",
            "excerpt": "Architectural guidelines for unified APIs and server-side templates.",
            "published": true,
            "category_id": cat_id
        }))
        .send()
        .await
        .unwrap();

    // 4. Verify post appears on rendered homepage
    let home_updated = client
        .get(format!("{base_url}/"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(home_updated.contains("Clean Web Architecture in Rust"));
    assert!(home_updated.contains("Rust Architecture"));

    // 5. Verify single post page renders with content and category
    let post_res = client
        .get(format!("{base_url}/posts/clean-web-architecture-in-rust"))
        .send()
        .await
        .unwrap();
    assert_eq!(post_res.status(), StatusCode::OK);
    let post_html = post_res.text().await.unwrap();
    assert!(post_html.contains("Clean Web Architecture in Rust"));
    assert!(post_html.contains("Using Axum, SQLite, and Tera provides high maintainability and speed."));
    assert!(post_html.contains("Rust Architecture"));

    // 6. Verify category filter page renders filtered posts
    let cat_filter_res = client
        .get(format!("{base_url}/category/rust-architecture"))
        .send()
        .await
        .unwrap();
    assert_eq!(cat_filter_res.status(), StatusCode::OK);
    let cat_html = cat_filter_res.text().await.unwrap();
    assert!(cat_html.contains("Clean Web Architecture in Rust"));

    // 7. Verify 404 page renders for nonexistent post
    let not_found_res = client
        .get(format!("{base_url}/posts/non-existent-article"))
        .send()
        .await
        .unwrap();
    assert_eq!(not_found_res.status(), StatusCode::NOT_FOUND);
    let not_found_html = not_found_res.text().await.unwrap();
    assert!(not_found_html.contains("404"));
    assert!(not_found_html.contains("Article or Page Not Found"));
}
