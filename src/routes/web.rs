use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tera::Context;

use crate::{
    auth::AppState,
    error::AppError,
    models::PostQuery,
    services,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(home_handler))
        .route("/category/{slug}", get(category_filter_handler))
        .route("/posts/{slug}", get(post_detail_handler))
        .route("/post/{slug}", get(post_detail_handler))
}

fn render(state: &AppState, template: &str, ctx: &Context) -> Result<Html<String>, AppError> {
    let rendered = state
        .tera
        .render(template, ctx)
        .map_err(|err| AppError::Internal(format!("Template render error ({template}): {err}")))?;

    Ok(Html(rendered))
}

fn render_404(state: &AppState, message: &str) -> Response {
    let mut ctx = Context::new();
    ctx.insert("title", "404 Not Found");
    ctx.insert("message", message);

    let html = state
        .tera
        .render("404.html", &ctx)
        .unwrap_or_else(|_| format!("<!DOCTYPE html><html><body><h1>404 Not Found</h1><p>{message}</p></body></html>"));

    (StatusCode::NOT_FOUND, Html(html)).into_response()
}

async fn home_handler(State(state): State<AppState>) -> Result<Response, AppError> {
    let posts = services::posts::list_posts(&state.pool, &PostQuery::default(), false).await?;
    let categories = services::categories::list_categories(&state.pool).await?;

    let mut ctx = Context::new();
    ctx.insert("title", "Oxide Blog - Home");
    ctx.insert("posts", &posts);
    ctx.insert("categories", &categories);
    ctx.insert("active_category", &None::<String>);

    Ok(render(&state, "index.html", &ctx)?.into_response())
}

async fn category_filter_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    let category = match services::categories::get_category_by_id_or_slug(&state.pool, &slug).await? {
        Some(cat) => cat,
        None => return Ok(render_404(&state, &format!("Category '{slug}' does not exist"))),
    };

    let query = PostQuery {
        category_slug: Some(slug.clone()),
        ..Default::default()
    };
    let posts = services::posts::list_posts(&state.pool, &query, false).await?;
    let categories = services::categories::list_categories(&state.pool).await?;

    let mut ctx = Context::new();
    ctx.insert("title", &format!("{} - Oxide Blog", category.name));
    ctx.insert("posts", &posts);
    ctx.insert("categories", &categories);
    ctx.insert("active_category", &Some(&category));

    Ok(render(&state, "index.html", &ctx)?.into_response())
}

async fn post_detail_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    let post = match services::posts::get_post_by_id_or_slug(&state.pool, &slug, false).await? {
        Some(p) => p,
        None => return Ok(render_404(&state, &format!("Post '{slug}' was not found"))),
    };

    let mut ctx = Context::new();
    ctx.insert("title", &format!("{} - Oxide Blog", post.title));
    ctx.insert("post", &post);

    Ok(render(&state, "post.html", &ctx)?.into_response())
}
