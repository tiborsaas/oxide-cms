use sqlx::SqlitePool;

use crate::{
    error::AppError,
    models::{slugify, CreateTagDto, Tag, TagQuery, UpdateTagDto},
};

pub async fn list_tags(
    pool: &SqlitePool,
    query: Option<&TagQuery>,
) -> Result<Vec<Tag>, AppError> {
    if let Some(q) = query {
        if let Some(search) = &q.search {
            let pattern = format!("%{search}%");
            let tags = sqlx::query_as::<_, Tag>(
                "SELECT id, name, slug, created_at, updated_at FROM tags WHERE name LIKE ? OR slug LIKE ? ORDER BY name ASC",
            )
            .bind(&pattern)
            .bind(&pattern)
            .fetch_all(pool)
            .await?;
            return Ok(tags);
        }
    }

    let tags = sqlx::query_as::<_, Tag>(
        "SELECT id, name, slug, created_at, updated_at FROM tags ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(tags)
}

pub async fn get_tag_by_id_or_slug(
    pool: &SqlitePool,
    id_or_slug: &str,
) -> Result<Option<Tag>, AppError> {
    let tag = if let Ok(id) = id_or_slug.parse::<i64>() {
        sqlx::query_as::<_, Tag>(
            "SELECT id, name, slug, created_at, updated_at FROM tags WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as::<_, Tag>(
            "SELECT id, name, slug, created_at, updated_at FROM tags WHERE slug = ?",
        )
        .bind(id_or_slug)
        .fetch_optional(pool)
        .await?
    };

    Ok(tag)
}

pub async fn create_tag(
    pool: &SqlitePool,
    payload: CreateTagDto,
) -> Result<Tag, AppError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Tag name cannot be empty".to_string()));
    }

    let slug = payload
        .slug
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| slugify(name));

    let tag = sqlx::query_as::<_, Tag>(
        r#"
        INSERT INTO tags (name, slug)
        VALUES (?, ?)
        RETURNING id, name, slug, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(&slug)
    .fetch_one(pool)
    .await?;

    Ok(tag)
}

pub async fn update_tag(
    pool: &SqlitePool,
    id: i64,
    payload: UpdateTagDto,
) -> Result<Tag, AppError> {
    let existing = sqlx::query_as::<_, Tag>(
        "SELECT id, name, slug, created_at, updated_at FROM tags WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Tag with id {id} not found")))?;

    let name = payload.name.as_deref().unwrap_or(&existing.name).trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Tag name cannot be empty".to_string()));
    }

    let slug = payload.slug.unwrap_or(existing.slug);

    let updated = sqlx::query_as::<_, Tag>(
        r#"
        UPDATE tags
        SET name = ?, slug = ?, updated_at = datetime('now')
        WHERE id = ?
        RETURNING id, name, slug, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(slug)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(updated)
}

pub async fn delete_tag(pool: &SqlitePool, id: i64) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Tag with id {id} not found")));
    }

    Ok(true)
}
