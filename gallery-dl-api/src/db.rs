use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tracing::{error, info};

/// Initialize the SQLite connection pool and run migrations.
pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    info!("Database initialized successfully");
    Ok(pool)
}

/// Run SQL migration files from the migrations directory.
async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let initial_sql = include_str!("../migrations/001_initial.sql");
    let unique_constraints_sql = include_str!("../migrations/003_add_unique_constraints.sql");

    // 001: Initial tables
    sqlx::raw_sql(initial_sql).execute(pool).await?;

    // 002: Image dimensions
    // We check if the column exists to avoid errors on re-run
    let _ = sqlx::query("ALTER TABLE images ADD COLUMN width INTEGER").execute(pool).await;
    let _ = sqlx::query("ALTER TABLE images ADD COLUMN height INTEGER").execute(pool).await;
    
    // 003: Unique constraints
    let _ = sqlx::raw_sql(unique_constraints_sql).execute(pool).await;

    // 004: Request title
    // Explicitly add column and log failure if it's not a "duplicate column" error
    if let Err(e) = sqlx::query("ALTER TABLE requests ADD COLUMN title TEXT").execute(pool).await {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
            error!(error = %msg, "Failed to apply title migration");
        }
    }

    // 005: Video enhancements (duration and progress)
    if let Err(e) = sqlx::query("ALTER TABLE videos ADD COLUMN duration_seconds REAL").execute(pool).await {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
            error!(error = %msg, "Failed to add duration_seconds to videos");
        }
    }
    
    if let Err(e) = sqlx::raw_sql(include_str!("../migrations/005_video_enhancements.sql")).execute(pool).await {
        let msg = e.to_string();
        // Ignore duplicate column errors from the script as well
        if !msg.contains("duplicate column name") {
             error!(error = %msg, "Failed to apply 005_video_enhancements migration");
        }
    }

    // 006: Video dimensions
    if let Err(e) = sqlx::raw_sql(include_str!("../migrations/006_video_dimensions.sql")).execute(pool).await {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
             error!(error = %msg, "Failed to apply 006_video_dimensions migration");
        }
    }

    // 007: People
    if let Err(e) = sqlx::raw_sql(include_str!("../migrations/007_people.sql")).execute(pool).await {
        let msg = e.to_string();
        if !msg.contains("already exists") && !msg.contains("duplicate column name") {
            error!(error = %msg, "Failed to apply 007_people migration");
        }
    }

    // 008: Image Colors
    if let Err(e) = sqlx::query("ALTER TABLE images ADD COLUMN top_colors TEXT").execute(pool).await {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
            error!(error = %msg, "Failed to add top_colors to images");
        }
    }

    // 009: Image favorites
    if let Err(e) = sqlx::query("ALTER TABLE images ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0").execute(pool).await {
        let msg = e.to_string();
        if !msg.contains("duplicate column name") {
            error!(error = %msg, "Failed to add is_favorite to images");
        }
    }

    // 010: Video title
    let has_title: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('videos') WHERE name = 'title'"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !has_title {
        if let Err(e) = sqlx::query("ALTER TABLE videos ADD COLUMN title TEXT").execute(pool).await {
            error!(error = %e, "Failed to add title to videos");
        }
    }

    info!("Migrations applied successfully");
    Ok(())
}
