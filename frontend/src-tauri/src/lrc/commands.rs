use super::{parse_lrc, LrcParseResult};
use crate::api::TranscriptSegment;
use crate::database::repositories::transcript::TranscriptsRepository;
use crate::state::AppState;
use chrono::Utc;
use log::{error as log_error, info as log_info};
use tauri::{AppHandle, Manager, Runtime, State};
use uuid::Uuid;

/// Import LRC file and create a new meeting with transcripts
#[tauri::command]
pub async fn api_import_lrc<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, AppState>,
    file_content: String,
) -> Result<String, String> {
    log_info!("api_import_lrc called");

    // Parse LRC content
    let parse_result = parse_lrc(&file_content)?;
    log_info!(
        "Parsed LRC file: {} lines, title: {:?}",
        parse_result.lines.len(),
        parse_result.metadata.title
    );

    // Generate meeting ID
    let meeting_id = format!("meeting-{}", Uuid::new_v4());

    // Use metadata title or default
    let meeting_title = parse_result
        .metadata
        .title
        .clone()
        .unwrap_or_else(|| format!("LRC Import {}", Utc::now().format("%Y-%m-%d %H:%M:%S")));

    // Convert LRC lines to TranscriptSegment
    let mut transcript_segments = Vec::new();
    let lines = &parse_result.lines;

    for (i, line) in lines.iter().enumerate() {
        let audio_start_time = line.time_seconds;

        // Calculate end time: use next line's start time, or add 5 seconds for last line
        let audio_end_time = if i + 1 < lines.len() {
            lines[i + 1].time_seconds
        } else {
            // Last line: add 5 seconds as default duration
            audio_start_time + 5.0
        };

        let duration = audio_end_time - audio_start_time;

        // Generate wall-clock timestamp (use current time as base)
        let timestamp = Utc::now().format("%H:%M:%S").to_string();

        transcript_segments.push(TranscriptSegment {
            id: format!("lrc-{}", i),
            text: line.text.clone(),
            timestamp,
            audio_start_time: Some(audio_start_time),
            audio_end_time: Some(audio_end_time),
            duration: Some(duration),
        });
    }

    log_info!(
        "Created {} transcript segments for meeting {}",
        transcript_segments.len(),
        meeting_id
    );

    // Save to database using existing repository
    let pool = &state.db_manager.pool();
    match TranscriptsRepository::save_transcript(
        pool,
        &meeting_title,
        &transcript_segments,
        None, // No folder path for LRC imports
    )
    .await
    {
        Ok(saved_meeting_id) => {
            log_info!(
                "Successfully imported LRC as meeting: {} ({})",
                meeting_title,
                saved_meeting_id
            );
            Ok(saved_meeting_id)
        }
        Err(e) => {
            log_error!("Failed to save LRC import to database: {}", e);
            Err(format!("Failed to save LRC import: {}", e))
        }
    }
}
