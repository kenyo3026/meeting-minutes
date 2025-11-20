use crate::database::repositories::{
    meeting::MeetingsRepository,
};
use crate::state::AppState;
use crate::summary::llm_client::{ChatMessage, stream_chat, LLMProvider};
use log::{error as log_error, info as log_info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub meeting_id: String,
    pub messages: Vec<ChatMessage>,
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub success: bool,
    pub message: String,
}

/// Send a chat message with streaming response
///
/// This command initiates a streaming chat conversation with the LLM.
/// The response will be streamed back via Tauri events:
/// - `llm:chat:token` for incremental content
/// - `llm:chat:done` for completion
/// - `llm:chat:error` for errors
#[tauri::command]
pub async fn chat_send_message<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    request: ChatRequest,
) -> Result<ChatResponse, String> {
    log_info!(
        "chat_send_message called for meeting_id: {}, provider: {}, model: {}",
        request.meeting_id,
        request.provider,
        request.model
    );

    // Parse provider
    let provider = match request.provider.as_str() {
        "openai" => LLMProvider::OpenAI,
        "claude" => LLMProvider::Claude,
        "groq" => LLMProvider::Groq,
        "ollama" => LLMProvider::Ollama,
        "openrouter" => LLMProvider::OpenRouter,
        "openai-compatible" => LLMProvider::OpenAICompatible,
        _ => return Err(format!("Unsupported provider: {}", request.provider)),
    };

    // Get API key from request or settings
    let api_key = if let Some(key) = request.api_key {
        key
    } else {
        // For Ollama, API key is optional
        if matches!(provider, LLMProvider::Ollama) {
            String::new()
        } else {
            return Err("API key is required for this provider".to_string());
        }
    };

    // Get endpoint (for Ollama and OpenAI-compatible)
    let endpoint = request.endpoint.clone();

    // Use meeting_id as request_id for streaming events
    let request_id = request.meeting_id.clone();

    // Create HTTP client
    let client = Client::new();

    // Spawn background task for streaming
    tauri::async_runtime::spawn(async move {
        log_info!("Starting streaming chat for request_id: {}", request_id);

        // Separate endpoints for Ollama and OpenAI-compatible
        let ollama_endpoint = if provider == LLMProvider::Ollama {
            endpoint.as_deref()
        } else {
            None
        };

        let openai_compatible_endpoint = if provider == LLMProvider::OpenAICompatible {
            endpoint.as_deref()
        } else {
            None
        };

        match stream_chat(
            app.clone(),
            &client,
            &provider,
            &request.model,
            &api_key,
            request.messages,
            request_id.clone(),
            ollama_endpoint,
            openai_compatible_endpoint,
            request.temperature,
            None, // top_p
            request.max_tokens,
        )
        .await
        {
            Ok(_) => {
                log_info!("Chat streaming completed successfully for: {}", request_id);
            }
            Err(e) => {
                log_error!("Chat streaming failed for {}: {}", request_id, e);
            }
        }
    });

    Ok(ChatResponse {
        success: true,
        message: "Chat message sent, streaming response...".to_string(),
    })
}

/// Get meeting context (title and transcript) for chat
#[tauri::command]
pub async fn chat_get_meeting_context<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingContext, String> {
    log_info!("chat_get_meeting_context called for meeting_id: {}", meeting_id);

    let pool = state.db_manager.pool();

    // Get meeting details (which includes transcripts)
    let meeting = MeetingsRepository::get_meeting(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to get meeting: {}", e))?
        .ok_or_else(|| format!("Meeting not found: {}", meeting_id))?;

    // Combine all transcript text
    let full_transcript = meeting.transcripts
        .iter()
        .map(|t| t.text.clone())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(MeetingContext {
        id: meeting.id.clone(),
        title: meeting.title.clone(),
        created_at: meeting.created_at.clone(),
        updated_at: meeting.updated_at.clone(),
        transcript: full_transcript,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeetingContext {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub transcript: String,
}

