use crate::api::MeetingDetails;
use crate::ollama::metadata::ModelMetadataCache;
use crate::summary::llm_client::{ChatMessage, LLMProvider};
use crate::summary::processor::{chunk_text, rough_token_count};
use once_cell::sync::Lazy;
use std::time::Duration;
use tracing::{info, warn};

// Global cache for model metadata (5 minute TTL) - same as summary service
static METADATA_CACHE: Lazy<ModelMetadataCache> = Lazy::new(|| {
    ModelMetadataCache::new(Duration::from_secs(300))
});

/// Gets the first chunk of transcript using the same chunking logic as summary/processor
/// This ensures KV cache compatibility - chat and summary will use the same first chunk
///
/// # Arguments
/// * `text` - Full transcript text
/// * `provider` - LLM provider
/// * `model_name` - Model name (for Ollama context size lookup)
/// * `ollama_endpoint` - Optional Ollama endpoint
/// * `openai_compatible_endpoint` - Optional OpenAI-compatible endpoint
///
/// # Returns
/// First chunk of transcript (or full text if no chunking needed)
pub async fn get_transcript_chunk_for_chat(
    text: &str,
    provider: &LLMProvider,
    model_name: &str,
    ollama_endpoint: Option<&str>,
    _openai_compatible_endpoint: Option<&str>,
) -> String {
    // Calculate token threshold - same logic as summary/processor
    // Only Ollama uses endpoint for metadata lookup, but we accept openai_compatible_endpoint
    // for consistency with summary/processor signature
    let token_threshold = if provider == &LLMProvider::Ollama {
        match METADATA_CACHE.get_or_fetch(model_name, ollama_endpoint).await {
            Ok(metadata) => {
                // Reserve 300 tokens for prompt overhead (same as summary)
                let optimal = metadata.context_size.saturating_sub(300);
                info!(
                    "✓ Using dynamic context for {}: {} tokens (chunk size: {})",
                    model_name, metadata.context_size, optimal
                );
                optimal
            }
            Err(e) => {
                warn!(
                    "⚠️ Failed to fetch context for {}: {}. Using default 4000",
                    model_name, e
                );
                4000  // Fallback to safe default
            }
        }
    } else {
        // Cloud providers (OpenAI, Claude, Groq, OpenAI-compatible) handle large contexts automatically
        100000  // Effectively unlimited for single-pass processing
    };

    // Chunk transcript using the same logic as summary/processor
    let total_tokens = rough_token_count(text);
    info!("Transcript length: {} tokens, threshold: {}", total_tokens, token_threshold);

    if provider != &LLMProvider::Ollama || total_tokens < token_threshold {
        // Single-pass: use full transcript (no chunking needed)
        info!("Using full transcript (tokens: {} < threshold: {})", total_tokens, token_threshold);
        text.to_string()
    } else {
        // Multi-level: chunk and use first chunk (same as summary's first chunk)
        info!(
            "Chunking transcript (tokens: {} >= threshold: {}), using first chunk",
            total_tokens, token_threshold
        );
        let chunks = chunk_text(text, token_threshold - 300, 100);
        if chunks.is_empty() {
            warn!("Chunking resulted in empty chunks, using full transcript");
            text.to_string()
        } else {
            info!("Using first chunk of {} chunks (length: {} chars)", chunks.len(), chunks[0].len());
            chunks[0].clone()
        }
    }
}

/// Builds the common prompt suffix for chat system prompts
fn build_chat_prompt_suffix(meeting_title: &str, meeting_date: &str, reference_text: &str) -> String {
    format!(
        r#"

You are a helpful AI assistant analyzing a meeting transcript. Here is the meeting information:

Title: {}
Date: {}

Please answer the user's questions based on the {} above. Be concise and accurate in your responses."#,
        meeting_title, meeting_date, reference_text
    )
}

/// Builds the system prompt for chat based on meeting context
/// Uses the same layout as summary/processor for KV cache compatibility
/// Automatically handles chunking using the same logic as summary/processor
///
/// # Arguments
/// * `meeting` - MeetingDetails object with title and date
/// * `transcript_text` - Full transcript text (will be chunked if needed)
/// * `provider` - LLM provider
/// * `model_name` - Model name (for Ollama context size lookup)
/// * `ollama_endpoint` - Optional Ollama endpoint
/// * `openai_compatible_endpoint` - Optional OpenAI-compatible endpoint
///
/// # Returns
/// Formatted system prompt string
pub async fn build_chat_system_prompt(
    meeting: &MeetingDetails,
    transcript_text: &str,
    provider: &LLMProvider,
    model_name: &str,
    ollama_endpoint: Option<&str>,
    _openai_compatible_endpoint: Option<&str>,
) -> String {
    info!("Building chat system prompt for meeting: {}", meeting.id);

    // Calculate token threshold (same logic as summary/processor)
    let token_threshold = if provider == &LLMProvider::Ollama {
        match METADATA_CACHE.get_or_fetch(model_name, ollama_endpoint).await {
            Ok(metadata) => metadata.context_size.saturating_sub(300),
            Err(_) => 4000,
        }
    } else {
        100000
    };

    let total_tokens = rough_token_count(transcript_text);

    // Format the meeting date
    let formatted_date = &meeting.created_at;

    // Use different prompt format based on token threshold to match summary/processor's KV cache
    let system_prompt = if provider != &LLMProvider::Ollama || total_tokens < token_threshold {
        // Token < threshold: Match summary's final prompt format (uses full transcript with <transcript_chunks>)
        info!("Chat using full transcript (tokens: {} < threshold: {}) - matching summary's final prompt", total_tokens, token_threshold);
        let prompt_suffix = build_chat_prompt_suffix(&meeting.title, formatted_date, "transcript");
        format!(
            r#"<transcript_chunks>
{}
</transcript_chunks>{}"#,
            transcript_text,
            prompt_suffix
        )
    } else {
        // Token >= threshold: Match summary's chunk prompt format (uses first chunk with <transcript_chunk>)
        info!("Chat using first chunk (tokens: {} >= threshold: {}) - matching summary's chunk prompt", total_tokens, token_threshold);
        let chunks = chunk_text(transcript_text, token_threshold - 300, 100);
        let first_chunk = if chunks.is_empty() {
            transcript_text.to_string()
        } else {
            chunks[0].clone()
        };
        let prompt_suffix = build_chat_prompt_suffix(&meeting.title, formatted_date, "transcript chunk");
        format!(
            r#"<transcript_chunk>
{}
</transcript_chunk>{}"#,
            first_chunk,
            prompt_suffix
        )
    };

    info!(
        "System prompt built successfully (length: {} chars)",
        system_prompt.len()
    );

    system_prompt
}

/// Builds complete message history for chat request
/// Automatically handles chunking using the same logic as summary/processor
///
/// # Arguments
/// * `meeting` - MeetingDetails object for context
/// * `transcript_text` - Full transcript text (will be chunked if needed)
/// * `provider` - LLM provider
/// * `model_name` - Model name (for Ollama context size lookup)
/// * `ollama_endpoint` - Optional Ollama endpoint
/// * `openai_compatible_endpoint` - Optional OpenAI-compatible endpoint
/// * `user_messages` - Previous chat messages (user and assistant)
/// * `current_message` - Current user message
///
/// # Returns
/// Complete message array with system prompt + history + current message
pub async fn build_chat_messages(
    meeting: &MeetingDetails,
    transcript_text: &str,
    provider: &LLMProvider,
    model_name: &str,
    ollama_endpoint: Option<&str>,
    openai_compatible_endpoint: Option<&str>,
    user_messages: Vec<ChatMessage>,
    current_message: &str,
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    // Add system prompt (automatically handles chunking)
    messages.push(ChatMessage {
        role: "system".to_string(),
        content: build_chat_system_prompt(
            meeting,
            transcript_text,
            provider,
            model_name,
            ollama_endpoint,
            openai_compatible_endpoint,
        )
        .await,
    });

    // Add message history
    messages.extend(user_messages);

    // Add current user message
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: current_message.to_string(),
    });

    info!("Built {} messages for chat request", messages.len());

    messages
}

/// Validates chat message structure
///
/// # Arguments
/// * `messages` - Messages to validate
///
/// # Returns
/// Ok(()) if valid, Err with description if invalid
pub fn validate_chat_messages(messages: &[ChatMessage]) -> Result<(), String> {
    if messages.is_empty() {
        return Err("Message list cannot be empty".to_string());
    }

    // Check for valid roles
    for msg in messages {
        if msg.role != "user" && msg.role != "assistant" && msg.role != "system" {
            return Err(format!("Invalid message role: {}", msg.role));
        }

        if msg.content.trim().is_empty() {
            return Err("Message content cannot be empty".to_string());
        }
    }

    Ok(())
}

