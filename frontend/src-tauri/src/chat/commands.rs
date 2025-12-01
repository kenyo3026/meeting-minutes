use crate::chat::processor;
use crate::database::repositories::{
    chat_message::ChatMessagesRepository,
    meeting::MeetingsRepository,
    transcript_chunk::TranscriptChunksRepository,
};
use crate::state::AppState;
use crate::summary::llm_client::{ChatMessage, ChatTemplateKwargs, stream_chat, LLMProvider};
use log::{error as log_error, info as log_info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub meeting_id: String,
    pub user_messages: Vec<ChatMessage>,  // Changed: only user/assistant history, no system
    pub current_message: String,           // New: current user message
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub repeat_last_n: Option<i32>,
    pub chat_template_kwargs: Option<ChatTemplateKwargs>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub success: bool,
    pub message: String,
}

/// Send a chat message with streaming response
///
/// This command initiates a streaming chat conversation with the LLM.
/// The system prompt is constructed automatically from meeting context.
/// The response will be streamed back via Tauri events:
/// - `llm:chat:token` for incremental content
/// - `llm:chat:done` for completion
/// - `llm:chat:error` for errors
#[tauri::command]
pub async fn api_chat_send_message<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    request: ChatRequest,
) -> Result<ChatResponse, String> {
    log_info!(
        "api_chat_send_message called for meeting_id: {}, provider: {}, model: {}",
        request.meeting_id,
        request.provider,
        request.model
    );

    // Get meeting context from database
    let pool = state.db_manager.pool();
    let meeting = MeetingsRepository::get_meeting(pool, &request.meeting_id)
        .await
        .map_err(|e| format!("Failed to get meeting: {}", e))?
        .ok_or_else(|| format!("Meeting not found: {}", request.meeting_id))?;

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

    // Get endpoint (for Ollama and OpenAI-compatible)
    let endpoint = request.endpoint.clone();

    // Get transcript text - prefer transcript_chunks for KV cache compatibility with summary
    // Fallback to combining transcripts table if transcript_chunks doesn't exist
    let transcript_text = match TranscriptChunksRepository::get_transcript_text(pool, &request.meeting_id).await {
        Ok(Some(text)) if !text.is_empty() => {
            log_info!("Using transcript from transcript_chunks (KV cache compatible)");
            text
        }
        Ok(Some(_)) | Ok(None) => {
            log_info!("transcript_chunks not found, falling back to transcripts table");
            // Fallback: combine transcripts from transcripts table
            meeting
                .transcripts
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<&str>>()
                .join("\n")
        }
        Err(e) => {
            log_error!("Failed to get transcript from transcript_chunks: {}, falling back", e);
            // Fallback: combine transcripts from transcripts table
            meeting
                .transcripts
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<&str>>()
                .join("\n")
        }
    };

    // Get endpoint (for Ollama and OpenAI-compatible)
    let endpoint = request.endpoint.clone();

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

    // Build complete message history with system prompt
    // Chunking logic is handled inside processor::build_chat_messages
    // This may return an error if transcript exceeds single-chunk limit
    let messages = processor::build_chat_messages(
        &meeting,
        &transcript_text,
        &provider,
        &request.model,
        ollama_endpoint,
        openai_compatible_endpoint,
        request.user_messages.clone(),
        &request.current_message,
    )
    .await?;

    // Validate messages
    processor::validate_chat_messages(&messages)?;

    log_info!(
        "Built {} messages for chat (including system prompt)",
        messages.len()
    );

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

    // Use meeting_id as request_id for streaming events
    let request_id = request.meeting_id.clone();

    // Set default parameters if not provided
    let temperature = request.temperature;
    let top_p = request.top_p;
    let max_tokens = request.max_tokens;
    let repeat_penalty = request.repeat_penalty;
    let repeat_last_n = request.repeat_last_n;
    let chat_template_kwargs = request.chat_template_kwargs.clone();

    // Create HTTP client
    let client = Client::new();

    // Spawn background task for streaming
    // Clone endpoints for move into async block
    let ollama_endpoint_clone = ollama_endpoint.map(|s| s.to_string());
    let openai_compatible_endpoint_clone = openai_compatible_endpoint.map(|s| s.to_string());

    tauri::async_runtime::spawn(async move {
        log_info!("Starting streaming chat for request_id: {}", request_id);

        // Use cloned endpoints
        let ollama_endpoint = ollama_endpoint_clone.as_deref();
        let openai_compatible_endpoint = openai_compatible_endpoint_clone.as_deref();

        match stream_chat(
            app.clone(),
            &client,
            &provider,
            &request.model,
            &api_key,
            messages,
            request_id.clone(),
            ollama_endpoint,
            openai_compatible_endpoint,
            temperature,
            top_p,
            max_tokens,
            repeat_penalty,
            repeat_last_n,
            chat_template_kwargs,
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
pub async fn api_chat_get_meeting_context<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingContext, String> {
    log_info!("api_chat_get_meeting_context called for meeting_id: {}", meeting_id);

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

/// Get chat history for a meeting
#[tauri::command]
pub async fn api_chat_get_history<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<ChatHistoryMessage>, String> {
    log_info!("api_chat_get_history called for meeting_id: {}", meeting_id);

    let pool = state.db_manager.pool();

    let messages = ChatMessagesRepository::get_chat_history(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to get chat history: {}", e))?;

    Ok(messages
        .into_iter()
        .map(|m| ChatHistoryMessage {
            id: m.id,
            role: m.role,
            content: m.content,
            timestamp: m.created_at.to_rfc3339(),
            ttft_us: m.ttft_us,
        })
        .collect())
}

/// Save a chat message to database
#[tauri::command]
pub async fn api_chat_save_message<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    role: String,
    content: String,
    ttft_us: Option<i64>,
) -> Result<String, String> {
    log_info!(
        "api_chat_save_message called for meeting_id: {}, role: {}",
        meeting_id,
        role
    );

    let pool = state.db_manager.pool();

    ChatMessagesRepository::save_message(pool, &meeting_id, &role, &content, ttft_us)
        .await
        .map_err(|e| format!("Failed to save chat message: {}", e))
}

/// Clear all chat history for a meeting
#[tauri::command]
pub async fn api_chat_clear_history<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<u64, String> {
    log_info!("api_chat_clear_history called for meeting_id: {}", meeting_id);

    let pool = state.db_manager.pool();

    ChatMessagesRepository::clear_history(pool, &meeting_id)
        .await
        .map_err(|e| format!("Failed to clear chat history: {}", e))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeetingContext {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub transcript: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatHistoryMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub ttft_us: Option<i64>,
}

