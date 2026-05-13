use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite:gallery_dl.db")
        .await?;

    let row = sqlx::query("PRAGMA table_info(requests)")
        .fetch_all(&pool)
        .await?;

    println!("Columns in 'requests' table:");
    for col in row {
        let name: String = col.get("name");
        println!("- {}", name);
    }

    Ok(())
}
