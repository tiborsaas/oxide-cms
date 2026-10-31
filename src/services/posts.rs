use sqlx::SqlitePool;

use crate::{
    error::AppError,
    models::{slugify, Category, CreatePostDto, Post, PostDetail, PostQuery, Tag, UpdatePostDto},
};

pub async fn get_tags_for_post(pool: &SqlitePool, post_id: i64) -> Result<Vec<Tag>, AppError> {
    let tags = sqlx::query_as::<_, Tag>(
        r#"
        SELECT t.id, t.name, t.slug, t.created_at, t.updated_at
        FROM tags t
        INNER JOIN post_tags pt ON pt.tag_id = t.id
        WHERE pt.post_id = ?
        ORDER BY t.name ASC
        "#,
    )
    .bind(post_id)
    .fetch_all(pool)
    .await?;

    Ok(tags)
}

pub async fn list_posts(
    pool: &SqlitePool,
    query: &PostQuery,
    is_admin: bool,
) -> Result<Vec<Post>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut builder = sqlx::QueryBuilder::new(
        r#"
        SELECT 
            p.id, p.title, p.slug, p.content, p.excerpt, p.published, p.category_id,
            c.name AS category_name, c.slug AS category_slug,
            p.created_at, p.updated_at
        FROM posts p
        LEFT JOIN categories c ON p.category_id = c.id
        WHERE 1=1 
        "#,
    );

    if !is_admin {
        builder.push(" AND p.published = 1");
    } else if let Some(pub_filter) = query.published {
        builder.push(" AND p.published = ");
        builder.push_bind(if pub_filter { 1 } else { 0 });
    }

    if let Some(cat_id) = query.category_id {
        builder.push(" AND p.category_id = ");
        builder.push_bind(cat_id);
    }

    if let Some(slug) = &query.category_slug {
        builder.push(" AND c.slug = ");
        builder.push_bind(slug);
    }

    if let Some(tag_id) = query.tag_id {
        builder.push(" AND EXISTS (SELECT 1 FROM post_tags pt WHERE pt.post_id = p.id AND pt.tag_id = ");
        builder.push_bind(tag_id);
        builder.push(")");
    }

    if let Some(tag_slug) = &query.tag_slug {
        builder.push(" AND EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON pt.tag_id = t.id WHERE pt.post_id = p.id AND t.slug = ");
        builder.push_bind(tag_slug);
        builder.push(")");
    }

    if let Some(search) = &query.search {
        let pattern = format!("%{search}%");
        builder.push(" AND (p.title LIKE ");
        builder.push_bind(pattern.clone());
        builder.push(" OR p.content LIKE ");
        builder.push_bind(pattern);
        builder.push(")");
    }

    builder.push(" ORDER BY p.created_at DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let query_stmt = builder.build_query_as::<Post>();
    let mut posts = query_stmt.fetch_all(pool).await?;

    if !posts.is_empty() {
        let post_ids: Vec<i64> = posts.iter().map(|p| p.id).collect();
        let mut tag_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT pt.post_id, t.id, t.name, t.slug, t.created_at, t.updated_at
            FROM tags t
            INNER JOIN post_tags pt ON pt.tag_id = t.id
            WHERE pt.post_id IN (
            "#,
        );
        let mut separated = tag_builder.separated(", ");
        for id in &post_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(") ORDER BY t.name ASC");

        #[derive(sqlx::FromRow)]
        struct PostTagRow {
            post_id: i64,
            id: i64,
            name: String,
            slug: String,
            created_at: String,
            updated_at: String,
        }

        let tag_rows = tag_builder.build_query_as::<PostTagRow>().fetch_all(pool).await?;
        let mut tags_by_post: std::collections::HashMap<i64, Vec<Tag>> = std::collections::HashMap::new();
        for row in tag_rows {
            tags_by_post.entry(row.post_id).or_default().push(Tag {
                id: row.id,
                name: row.name,
                slug: row.slug,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }

        for post in &mut posts {
            if let Some(tags) = tags_by_post.remove(&post.id) {
                post.tags = tags;
            }
        }
    }

    Ok(posts)
}

pub async fn get_post_by_id_or_slug(
    pool: &SqlitePool,
    id_or_slug: &str,
    is_admin: bool,
) -> Result<Option<PostDetail>, AppError> {
    let post = if let Ok(id) = id_or_slug.parse::<i64>() {
        sqlx::query_as::<_, Post>(
            r#"
            SELECT 
                p.id, p.title, p.slug, p.content, p.excerpt, p.published, p.category_id,
                c.name AS category_name, c.slug AS category_slug,
                p.created_at, p.updated_at
            FROM posts p
            LEFT JOIN categories c ON p.category_id = c.id
            WHERE p.id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as::<_, Post>(
            r#"
            SELECT 
                p.id, p.title, p.slug, p.content, p.excerpt, p.published, p.category_id,
                c.name AS category_name, c.slug AS category_slug,
                p.created_at, p.updated_at
            FROM posts p
            LEFT JOIN categories c ON p.category_id = c.id
            WHERE p.slug = ?
            "#,
        )
        .bind(id_or_slug)
        .fetch_optional(pool)
        .await?
    };

    let post = match post {
        Some(p) => p,
        None => return Ok(None),
    };

    if !post.published && !is_admin {
        return Ok(None);
    }

    let category = if let Some(cat_id) = post.category_id {
        sqlx::query_as::<_, Category>(
            "SELECT id, name, slug, description, created_at, updated_at FROM categories WHERE id = ?",
        )
        .bind(cat_id)
        .fetch_optional(pool)
        .await?
    } else {
        None
    };

    let tags = get_tags_for_post(pool, post.id).await?;

    Ok(Some(PostDetail {
        id: post.id,
        title: post.title,
        slug: post.slug,
        content: post.content,
        excerpt: post.excerpt,
        published: post.published,
        category,
        tags,
        created_at: post.created_at,
        updated_at: post.updated_at,
    }))
}

pub async fn create_post(
    pool: &SqlitePool,
    payload: CreatePostDto,
) -> Result<Post, AppError> {
    let title = payload.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("Post title cannot be empty".to_string()));
    }

    let slug = payload
        .slug
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| slugify(title));

    if let Some(cat_id) = payload.category_id {
        let exists = sqlx::query("SELECT 1 FROM categories WHERE id = ?")
            .bind(cat_id)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::BadRequest(format!(
                "Category with id {cat_id} does not exist"
            )));
        }
    }

    if let Some(ref tag_ids) = payload.tag_ids {
        for &tid in tag_ids {
            let exists = sqlx::query("SELECT 1 FROM tags WHERE id = ?")
                .bind(tid)
                .fetch_optional(pool)
                .await?;
            if exists.is_none() {
                return Err(AppError::BadRequest(format!(
                    "Tag with id {tid} does not exist"
                )));
            }
        }
    }

    let inserted = sqlx::query(
        r#"
        INSERT INTO posts (title, slug, content, excerpt, published, category_id)
        VALUES (?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(title)
    .bind(&slug)
    .bind(&payload.content)
    .bind(payload.excerpt.as_deref().map(str::trim))
    .bind(payload.published)
    .bind(payload.category_id)
    .fetch_one(pool)
    .await?;

    let id: i64 = sqlx::Row::get(&inserted, "id");

    if let Some(tag_ids) = payload.tag_ids {
        for tid in tag_ids {
            sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
                .bind(id)
                .bind(tid)
                .execute(pool)
                .await?;
        }
    }

    let mut post = sqlx::query_as::<_, Post>(
        r#"
        SELECT 
            p.id, p.title, p.slug, p.content, p.excerpt, p.published, p.category_id,
            c.name AS category_name, c.slug AS category_slug,
            p.created_at, p.updated_at
        FROM posts p
        LEFT JOIN categories c ON p.category_id = c.id
        WHERE p.id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    post.tags = get_tags_for_post(pool, id).await?;

    Ok(post)
}

pub async fn update_post(
    pool: &SqlitePool,
    id: i64,
    payload: UpdatePostDto,
) -> Result<Post, AppError> {
    let existing = sqlx::query_as::<_, Post>(
        r#"
        SELECT 
            p.id, p.title, p.slug, p.content, p.excerpt, p.published, p.category_id,
            c.name AS category_name, c.slug AS category_slug,
            p.created_at, p.updated_at
        FROM posts p
        LEFT JOIN categories c ON p.category_id = c.id
        WHERE p.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Post with id {id} not found")))?;

    let title = payload.title.as_deref().unwrap_or(&existing.title).trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("Post title cannot be empty".to_string()));
    }

    let slug = payload.slug.unwrap_or(existing.slug);
    let content = payload.content.as_deref().unwrap_or(&existing.content);
    let excerpt = payload.excerpt.or(existing.excerpt);
    let published = payload.published.unwrap_or(existing.published);
    let category_id = payload.category_id.or(existing.category_id);

    if let Some(cat_id) = category_id {
        let exists = sqlx::query("SELECT 1 FROM categories WHERE id = ?")
            .bind(cat_id)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::BadRequest(format!(
                "Category with id {cat_id} does not exist"
            )));
        }
    }

    if let Some(ref tag_ids) = payload.tag_ids {
        for &tid in tag_ids {
            let exists = sqlx::query("SELECT 1 FROM tags WHERE id = ?")
                .bind(tid)
                .fetch_optional(pool)
                .await?;
            if exists.is_none() {
                return Err(AppError::BadRequest(format!(
                    "Tag with id {tid} does not exist"
                )));
            }
        }
    }

    sqlx::query(
        r#"
        UPDATE posts
        SET title = ?, slug = ?, content = ?, excerpt = ?, published = ?, category_id = ?, updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(title)
    .bind(slug)
    .bind(content)
    .bind(excerpt)
    .bind(published)
    .bind(category_id)
    .bind(id)
    .execute(pool)
    .await?;

    if let Some(tag_ids) = payload.tag_ids {
        sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
            .bind(id)
            .execute(pool)
            .await?;

        for tid in tag_ids {
            sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
                .bind(id)
                .bind(tid)
                .execute(pool)
                .await?;
        }
    }

    let mut updated = sqlx::query_as::<_, Post>(
        r#"
        SELECT 
            p.id, p.title, p.slug, p.content, p.excerpt, p.published, p.category_id,
            c.name AS category_name, c.slug AS category_slug,
            p.created_at, p.updated_at
        FROM posts p
        LEFT JOIN categories c ON p.category_id = c.id
        WHERE p.id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    updated.tags = get_tags_for_post(pool, id).await?;

    Ok(updated)
}

pub async fn delete_post(pool: &SqlitePool, id: i64) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Post with id {id} not found")));
    }

    Ok(true)
}
