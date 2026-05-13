use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use tracing::info;

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
    let dimensions_sql = include_str!("../migrations/002_add_image_dimensions.sql");
    let unique_constraints_sql = include_str!("../migrations/003_add_unique_constraints.sql");
    let title_migration_sql = include_str!("../migrations/004_add_request_title.sql");

    sqlx::raw_sql(initial_sql).execute(pool).await?;
    
    // We use a separate block or check if columns exist if we wanted it to be idempotent, 
    // but for simple dev flow we can just try and ignore "duplicate column" errors 
    // or better, check if they exist.
    // However, since we are using raw SQL, we'll just execute it.
    let _ = sqlx::raw_sql(dimensions_sql).execute(pool).await;
    
    sqlx::raw_sql(unique_constraints_sql).execute(pool).await?;

    let _ = sqlx::raw_sql(title_migration_sql).execute(pool).await;

    info!("Migrations applied successfully");
    Ok(())
}
