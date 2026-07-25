/// Run table and index initialization for the application
pub async fn init_db(
    pool: &sqlx::PgPool,
    config: &crate::config::Config,
) -> Result<(), sqlx::Error> {
    println!("Info | Initializing database schema...");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pastes (
            id VARCHAR(64) PRIMARY KEY,
            content TEXT NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
            expires_at TIMESTAMP WITH TIME ZONE NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    // Create an expression index on md5(content) for fast duplicate content checks
    sqlx::query("CREATE INDEX IF NOT EXISTS pastes_content_md5_idx ON pastes (md5(content))")
        .execute(pool)
        .await?;

    // Create an index on expires_at for fast background cleanup
    sqlx::query("CREATE INDEX IF NOT EXISTS pastes_expires_at_idx ON pastes (expires_at)")
        .execute(pool)
        .await?;

    // Dynamic database schema management for optional password protection
    if config.security.password_protection_enabled {
        println!("Info | Password protection enabled. Ensuring password_hash column exists...");
        sqlx::query("ALTER TABLE pastes ADD COLUMN IF NOT EXISTS password_hash VARCHAR(255)")
            .execute(pool)
            .await?;
    } else {
        println!("Info | Password protection disabled. Cleaning up password columns...");
        sqlx::query("ALTER TABLE pastes DROP COLUMN IF EXISTS password_hash")
            .execute(pool)
            .await?;
    }

    println!("Success | Database schema initialized");
    Ok(())
}
