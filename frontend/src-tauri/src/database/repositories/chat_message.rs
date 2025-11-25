use crate::database::models::ChatMessageModel;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{error, info};
use uuid::Uuid;

pub struct ChatMessagesRepository;

impl ChatMessagesRepository {
    /// Gets all chat messages for a meeting, ordered by creation time
    pub async fn get_chat_history(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Vec<ChatMessageModel>, sqlx::Error> {
        info!("Getting chat history for meeting_id: {}", meeting_id);

        sqlx::query_as::<_, ChatMessageModel>(
            "SELECT * FROM chat_messages WHERE meeting_id = ? ORDER BY created_at ASC"
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
    }

    /// Saves a single chat message
    /// Uses a transaction to ensure data consistency and verify meeting exists
    pub async fn save_message(
        pool: &SqlitePool,
        meeting_id: &str,
        role: &str,
        content: &str,
        ttft_us: Option<i64>,
    ) -> Result<String, sqlx::Error> {
        let mut transaction = pool.begin().await?;

        // Verify meeting exists before inserting message
        let meeting_exists: bool = sqlx::query("SELECT 1 FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();

        if !meeting_exists {
            info!(
                "Attempted to save chat message for a non-existent meeting_id: {}",
                meeting_id
            );
            transaction.rollback().await?;
            return Err(sqlx::Error::RowNotFound);
        }

        let message_id = format!("chat-msg-{}", Uuid::new_v4());
        let now = Utc::now();

        info!(
            "Saving chat message for meeting_id: {}, role: {}",
            meeting_id, role
        );

        sqlx::query(
            r#"
            INSERT INTO chat_messages (id, meeting_id, role, content, created_at, ttft_us)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&message_id)
        .bind(meeting_id)
        .bind(role)
        .bind(content)
        .bind(now)
        .bind(ttft_us)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;

        info!(
            "Successfully saved chat message {} for meeting_id: {}",
            message_id, meeting_id
        );

        Ok(message_id)
    }

    /// Clears all chat messages for a meeting
    /// Uses a transaction to ensure data consistency
    pub async fn clear_history(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<u64, sqlx::Error> {
        info!("Clearing chat history for meeting_id: {}", meeting_id);

        let mut transaction = pool.begin().await?;

        let result = sqlx::query("DELETE FROM chat_messages WHERE meeting_id = ?")
            .bind(meeting_id)
            .execute(&mut *transaction)
            .await?;

        transaction.commit().await?;

        info!(
            "Successfully cleared {} chat messages for meeting_id: {}",
            result.rows_affected(),
            meeting_id
        );

        Ok(result.rows_affected())
    }

    /// Deletes a specific chat message
    pub async fn delete_message(
        pool: &SqlitePool,
        message_id: &str,
    ) -> Result<bool, sqlx::Error> {
        info!("Deleting chat message: {}", message_id);

        let result = sqlx::query("DELETE FROM chat_messages WHERE id = ?")
            .bind(message_id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

