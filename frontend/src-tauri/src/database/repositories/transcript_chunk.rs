// src/database/repo/transcript_chunks.rs

use chrono::Utc;
use log::info as log_info;
use sqlx::SqlitePool;
pub struct TranscriptChunksRepository;

impl TranscriptChunksRepository {
    /// Saves the full transcript text and processing parameters.
    pub async fn save_transcript_data(
        pool: &SqlitePool,
        meeting_id: &str,
        text: &str,
        model: &str,
        model_name: &str,
        chunk_size: i32,
        overlap: i32,
    ) -> Result<(), sqlx::Error> {
        log_info!(
            "Saving transcript data to transcript_chunks for meeting_id: {}",
            meeting_id
        );
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO transcript_chunks (meeting_id, transcript_text, model, model_name, chunk_size, overlap, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(meeting_id) DO UPDATE SET
                transcript_text = excluded.transcript_text,
                model = excluded.model,
                model_name = excluded.model_name,
                chunk_size = excluded.chunk_size,
                overlap = excluded.overlap,
                created_at = excluded.created_at
            "#
        )
        .bind(meeting_id)
        .bind(text)
        .bind(model)
        .bind(model_name)
        .bind(chunk_size)
        .bind(overlap)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Gets the full transcript text for a meeting from transcript_chunks table.
    ///
    /// # Arguments
    /// * `pool` - SQLx connection pool
    /// * `meeting_id` - Meeting identifier
    ///
    /// # Returns
    /// Option<String> - Full transcript text if found, None otherwise
    pub async fn get_transcript_text(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query_scalar::<_, Option<String>>(
            "SELECT transcript_text FROM transcript_chunks WHERE meeting_id = ?"
        )
        .bind(meeting_id)
        .fetch_optional(pool)
        .await?;

        Ok(result.flatten())
    }

    /// Updates only the transcript_text field, preserving other configuration.
    /// Creates the record if it doesn't exist (with default values for other fields).
    ///
    /// # Arguments
    /// * `pool` - SQLx connection pool
    /// * `meeting_id` - Meeting identifier
    /// * `text` - Full transcript text to update
    ///
    /// # Returns
    /// Result<(), sqlx::Error> - Ok if update/insert succeeded
    pub async fn update_transcript_text_only(
        pool: &SqlitePool,
        meeting_id: &str,
        text: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        
        // Use INSERT OR REPLACE to handle both insert and update cases
        // If record doesn't exist, create with default values for required fields
        sqlx::query(
            r#"
            INSERT INTO transcript_chunks (meeting_id, transcript_text, model, model_name, created_at)
            VALUES (?, ?, 'unknown', 'unknown', ?)
            ON CONFLICT(meeting_id) DO UPDATE SET
                transcript_text = excluded.transcript_text,
                created_at = excluded.created_at
            "#
        )
        .bind(meeting_id)
        .bind(text)
        .bind(now)
        .execute(pool)
        .await?;

        log_info!("✅ Updated transcript_text in transcript_chunks for meeting_id: {}", meeting_id);
        Ok(())
    }
}
