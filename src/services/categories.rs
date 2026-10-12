use sqlx::SqlitePool;

use crate::{
    error::AppError,
    models::{slugify, Category, CreateCategoryDto, UpdateCategoryDto},
};

pub async fn list_categories(pool: &SqlitePool) -> Result<Vec<Category>, AppError> {
    let categories = sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description, created_at, updated_at FROM categories ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(categories)
}

pub async fn get_category_by_id_or_slug(
    pool: &SqlitePool,
    id_or_slug: &str,
) -> Result<Option<Category>, AppError> {
    let category = if let Ok(id) = id_or_slug.parse::<i64>() {
        sqlx::query_as::<_, Category>(
            "SELECT id, name, slug, description, created_at, updated_at FROM categories WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as::<_, Category>(
            "SELECT id, name, slug, description, created_at, updated_at FROM categories WHERE slug = ?",
        )
        .bind(id_or_slug)
        .fetch_optional(pool)
        .await?
    };

    Ok(category)
}

pub async fn create_category(
    pool: &SqlitePool,
    payload: CreateCategoryDto,
) -> Result<Category, AppError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Category name cannot be empty".to_string()));
    }

    let slug = payload
        .slug
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| slugify(name));

    let category = sqlx::query_as::<_, Category>(
        r#"
        INSERT INTO categories (name, slug, description)
        VALUES (?, ?, ?)
        RETURNING id, name, slug, description, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(&slug)
    .bind(payload.description.as_deref().map(str::trim))
    .fetch_one(pool)
    .await?;

    Ok(category)
}

pub async fn update_category(
    pool: &SqlitePool,
    id: i64,
    payload: UpdateCategoryDto,
) -> Result<Category, AppError> {
    let existing = sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description, created_at, updated_at FROM categories WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Category with id {id} not found")))?;

    let name = payload.name.as_deref().unwrap_or(&existing.name).trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Category name cannot be empty".to_string()));
    }

    let slug = payload.slug.unwrap_or(existing.slug);
    let description = payload.description.or(existing.description);

    let updated = sqlx::query_as::<_, Category>(
        r#"
        UPDATE categories
        SET name = ?, slug = ?, description = ?, updated_at = datetime('now')
        WHERE id = ?
        RETURNING id, name, slug, description, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(slug)
    .bind(description)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(updated)
}

pub async fn delete_category(pool: &SqlitePool, id: i64) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Category with id {id} not found")));
    }

    Ok(true)
}
