use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};
use futures_util::StreamExt;
use tauri::{Emitter, Runtime};

// ============================================================================
// STREAMING EVENT PAYLOADS (Tauri Events)
// ============================================================================

/// Streaming token event payload (sent incrementally)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTokenPayload {
    pub request_id: String,
    pub content_delta: String,
}

/// Streaming completion event payload (sent once at the end)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDonePayload {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<StreamUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Time to first token in microseconds (1 ms = 1000 μs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_us: Option<u64>,
}

/// Usage statistics for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}

/// Streaming error event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamErrorPayload {
    pub request_id: String,
    pub message: String,
}

// ============================================================================
// TTFT LOGGING UTILITIES
// ============================================================================

/// Log unified TTFT metrics in a clean 3-line format
///
/// # Arguments
/// * `ttft_us` - Time to first token in microseconds
/// * `total_time_us` - Total streaming time in microseconds
fn log_ttft_metrics(ttft_us: Option<u64>, total_time_us: u64) {
    if let Some(ttft) = ttft_us {
        let t1_to_tn_us = total_time_us - ttft;
        let ttft_ms = ttft as f64 / 1000.0;
        let t1_to_tn_ms = t1_to_tn_us as f64 / 1000.0;
        let total_ms = total_time_us as f64 / 1000.0;
        let ttft_ratio = (ttft as f64 / total_time_us as f64) * 100.0;

        info!("⏱️  Timeline: t0 (start) → t1 (first token) → tn (complete)");
        info!("⏱️  Elapsed: t0→t1={:.0}ms | t1→tn={:.0}ms | t0→tn={:.0}ms",
              ttft_ms, t1_to_tn_ms, total_ms);
        info!("⏱️  TTFT: {:.0}ms ({:.1}% of total)", ttft_ms, ttft_ratio);
    }
}

// ============================================================================
// CHAT MESSAGE STRUCTURES
// ============================================================================

// Generic structure for OpenAI-compatible API chat messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// Generic structure for OpenAI-compatible API chat requests (non-streaming)
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

// Generic structure for OpenAI-compatible API chat requests (with streaming option)
#[derive(Debug, Serialize)]
pub struct StreamingChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

// Generic structure for OpenAI-compatible API chat responses
#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
pub struct Choice {
    pub message: MessageContent,
}

#[derive(Deserialize, Debug)]
pub struct MessageContent {
    pub content: String,
}

// Claude-specific request structure
#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<ChatMessage>,
}

// Claude-specific response structure
#[derive(Deserialize, Debug)]
pub struct ClaudeChatResponse {
    pub content: Vec<ClaudeChatContent>,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeChatContent {
    pub text: String,
}

// ============================================================================
// STREAMING RESPONSE STRUCTURES
// ============================================================================

/// OpenAI-compatible streaming response chunk
#[derive(Deserialize, Debug)]
pub struct StreamChatChunk {
    #[serde(default)]
    pub choices: Vec<StreamChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<StreamUsageResponse>,
    /// Unix timestamp (seconds since epoch) when this chunk was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    pub delta: StreamDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct StreamDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct StreamUsageResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}

/// Claude-specific streaming event types
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ClaudeStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: ClaudeMessageStart },
    #[serde(rename = "content_block_start")]
    ContentBlockStart { content_block: ClaudeContentBlock },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: ClaudeContentDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop,
    #[serde(rename = "message_delta")]
    MessageDelta { delta: ClaudeMessageDelta, usage: ClaudeUsageDelta },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "error")]
    Error { error: ClaudeError },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeMessageStart {
    pub id: String,
    pub model: String,
    pub role: String,
    pub usage: ClaudeUsageDelta,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeContentDelta {
    #[serde(rename = "type")]
    pub delta_type: String,
    pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeMessageDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeUsageDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Claude-specific streaming request
#[derive(Debug, Serialize)]
pub struct ClaudeStreamRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

/// LLM Provider enumeration for multi-provider support
#[derive(Debug, Clone, PartialEq)]
pub enum LLMProvider {
    OpenAI,
    Claude,
    Groq,
    Ollama,
    OpenRouter,
    OpenAICompatible,
}

impl LLMProvider {
    /// Parse provider from string (case-insensitive)
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(Self::OpenAI),
            "claude" => Ok(Self::Claude),
            "groq" => Ok(Self::Groq),
            "ollama" => Ok(Self::Ollama),
            "openrouter" => Ok(Self::OpenRouter),
            "openai-compatible" => Ok(Self::OpenAICompatible),
            _ => Err(format!("Unsupported LLM provider: {}", s)),
        }
    }
}

/// Summary generation result with timing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryResult {
    pub content: String,
    pub ttft_us: Option<u64>,    // Time to first token in microseconds
    pub total_time_us: u64,       // Total generation time in microseconds (tn - t0)
}

/// Generates a summary using the specified LLM provider
///
/// # Arguments
/// * `client` - Reqwest HTTP client (reused for performance)
/// * `provider` - The LLM provider to use
/// * `model_name` - The specific model to use (e.g., "gpt-4", "claude-3-opus")
/// * `api_key` - API key for the provider (not needed for Ollama)
/// * `system_prompt` - System instructions for the LLM
/// * `user_prompt` - User query/content to process
/// * `ollama_endpoint` - Optional custom Ollama endpoint (defaults to localhost:11434)
/// * `openai_compatible_endpoint` - Optional custom OpenAI-compatible endpoint
/// * `stream` - Whether to use streaming mode (enables TTFT tracking)
///
/// # Returns
/// SummaryResult with content, ttft_us (if stream=true), and total_time_us
pub async fn generate_summary(
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    ollama_endpoint: Option<&str>,
    openai_compatible_endpoint: Option<&str>,
    stream: bool,
) -> Result<SummaryResult, String> {
    use std::time::Instant;

    // Route to streaming or non-streaming based on parameter
    if stream {
        _generate_summary_streaming(
            client,
            provider,
            model_name,
            api_key,
            system_prompt,
            user_prompt,
            ollama_endpoint,
            openai_compatible_endpoint,
        )
        .await
    } else {
        _generate_summary_non_streaming(
            client,
            provider,
            model_name,
            api_key,
            system_prompt,
            user_prompt,
            ollama_endpoint,
            openai_compatible_endpoint,
        )
        .await
    }
}

/// Internal: Non-streaming summary generation (original logic, ttft_us is always None)
async fn _generate_summary_non_streaming(
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    ollama_endpoint: Option<&str>,
    openai_compatible_endpoint: Option<&str>,
) -> Result<SummaryResult, String> {
    use std::time::Instant;
    let request_start_time = Instant::now();

    let (api_url, mut headers) = match provider {
        LLMProvider::OpenAI => (
            "https://api.openai.com/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Groq => (
            "https://api.groq.com/openai/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::OpenRouter => (
            "https://openrouter.ai/api/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Ollama => {
            let host = ollama_endpoint
                .map(|s| s.to_string())
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            (
                format!("{}/v1/chat/completions", host),
                header::HeaderMap::new(),
            )
        },
        LLMProvider::Claude => {
            let mut header_map = header::HeaderMap::new();
            header_map.insert(
                "x-api-key",
                api_key
                    .parse()
                    .map_err(|_| "Invalid API key format".to_string())?,
            );
            header_map.insert(
                "anthropic-version",
                "2023-06-01"
                    .parse()
                    .map_err(|_| "Invalid anthropic version".to_string())?,
            );
            ("https://api.anthropic.com/v1/messages".to_string(), header_map)
        },
        LLMProvider::OpenAICompatible => {
            let base_url = openai_compatible_endpoint
                .ok_or("OpenAI Compatible endpoint not configured")?
                .trim_end_matches('/');
            (
                format!("{}/chat/completions", base_url),
                header::HeaderMap::new(),
            )
        }
    };

    // Add authorization header for non-Claude providers
    if provider != &LLMProvider::Claude {
        // OpenAI Compatible might not need API key (local services)
        if !api_key.is_empty() || provider != &LLMProvider::OpenAICompatible {
            headers.insert(
                header::AUTHORIZATION,
                format!("Bearer {}", api_key)
                    .parse()
                    .map_err(|_| "Invalid authorization header".to_string())?,
            );
        }
    }
    headers.insert(
        header::CONTENT_TYPE,
        "application/json"
            .parse()
            .map_err(|_| "Invalid content type".to_string())?,
    );

    // Build request body based on provider
    let request_body = if provider != &LLMProvider::Claude {
        serde_json::json!(ChatRequest {
            model: model_name.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                }
            ],
        })
    } else {
        serde_json::json!(ClaudeRequest {
            system: system_prompt.to_string(),
            model: model_name.to_string(),
            max_tokens: 2048,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            }]
        })
    };

    info!("🐞 LLM Request to {}: model={} (non-streaming)", provider_name(provider), model_name);

    // Send request
    let response = client
        .post(api_url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to LLM: {}", e))?;

    if !response.status().is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("LLM API request failed: {}", error_body));
    }

    // Parse response based on provider
    if provider == &LLMProvider::Claude {
        let chat_response = response
            .json::<ClaudeChatResponse>()
            .await
            .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

        info!("🐞 LLM Response received from Claude");

        let content = chat_response
            .content
            .get(0)
            .ok_or("No content in LLM response")?
            .text
            .trim();

        let total_time_us = request_start_time.elapsed().as_micros() as u64;
        info!("⏱️ Summary generation total time: {:.2}ms", total_time_us as f64 / 1000.0);

        // For non-streaming, we don't have TTFT but can measure total time
        Ok(SummaryResult {
            content: content.to_string(),
            ttft_us: None, // Non-streaming doesn't capture TTFT
            total_time_us,
        })
    } else {
        let chat_response = response
            .json::<ChatResponse>()
            .await
            .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

        info!("🐞 LLM Response received from {}", provider_name(provider));

        let content = chat_response
            .choices
            .get(0)
            .ok_or("No content in LLM response")?
            .message
            .content
            .trim();

        let total_time_us = request_start_time.elapsed().as_micros() as u64;
        info!("⏱️ Summary generation total time: {:.2}ms", total_time_us as f64 / 1000.0);

        Ok(SummaryResult {
            content: content.to_string(),
            ttft_us: None, // Non-streaming doesn't capture TTFT
            total_time_us,
        })
    }
}

/// Internal: Streaming summary generation (consumes stream internally, captures TTFT)
///
/// This function uses streaming to capture TTFT metrics but accumulates all tokens
/// internally before returning, so the caller receives a complete response.
async fn generate_summary_streaming(
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    ollama_endpoint: Option<&str>,
    openai_compatible_endpoint: Option<&str>,
) -> Result<SummaryResult, String> {
    use std::time::Instant;

    // Route to appropriate streaming handler based on provider
    if provider == &LLMProvider::Claude {
        _generate_summary_streaming_claude(
            client,
            model_name,
            api_key,
            system_prompt,
            user_prompt,
        )
        .await
    } else {
        _generate_summary_streaming(
            client,
            provider,
            model_name,
            api_key,
            system_prompt,
            user_prompt,
            ollama_endpoint,
            openai_compatible_endpoint,
        )
        .await
    }
}

/// Internal: Streaming handler for OpenAI-compatible providers (reuses chat streaming logic)
async fn _generate_summary_streaming(
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    ollama_endpoint: Option<&str>,
    openai_compatible_endpoint: Option<&str>,
) -> Result<SummaryResult, String> {
    use std::time::Instant;
    let request_start_time = Instant::now();

    // Determine API endpoint (reuse logic from stream_chat_openai_compatible)
    let (api_url, mut headers) = match provider {
        LLMProvider::OpenAI => (
            "https://api.openai.com/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Groq => (
            "https://api.groq.com/openai/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::OpenRouter => (
            "https://openrouter.ai/api/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Ollama => {
            let host = ollama_endpoint
                .map(|s| s.to_string())
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            (
                format!("{}/v1/chat/completions", host),
                header::HeaderMap::new(),
            )
        }
        LLMProvider::OpenAICompatible => {
            let base_url = openai_compatible_endpoint
                .ok_or("OpenAI Compatible endpoint not configured")?
                .trim_end_matches('/');
            (
                format!("{}/chat/completions", base_url),
                header::HeaderMap::new(),
            )
        }
        _ => return Err("Unsupported provider for OpenAI-compatible streaming".to_string()),
    };

    // Add authorization header
    if !api_key.is_empty() || provider != &LLMProvider::OpenAICompatible {
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", api_key)
                .parse()
                .map_err(|_| "Invalid authorization header".to_string())?,
        );
    }
    headers.insert(
        header::CONTENT_TYPE,
        "application/json"
            .parse()
            .map_err(|_| "Invalid content type".to_string())?,
    );

    // Build streaming request body
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        },
    ];

    let request_body = StreamingChatRequest {
        model: model_name.to_string(),
        messages,
        stream: true,
        temperature: None,
        top_p: None,
        max_tokens: Some(2048),
    };

    info!("🚀 Sending streaming summary request to {}: {}", provider_name(provider), api_url);

    // Send request
    let response = client
        .post(&api_url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to send streaming request: {}", e))?;

    if !response.status().is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Streaming API request failed: {}", error_body));
    }

    // Process streaming response - accumulate tokens internally
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut accumulated_content = String::new();
    let mut ttft_us: Option<u64> = None;
    let mut first_token_received = false;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                // Convert bytes to string
                let chunk_str = String::from_utf8_lossy(&chunk);
                buffer.push_str(&chunk_str);

                // Process complete lines (SSE format: "data: {...}\n\n")
                while let Some(line_end) = buffer.find("\n\n") {
                    let line = buffer[..line_end].to_string();
                    buffer = buffer[line_end + 2..].to_string();

                    // Parse SSE line
                    for sse_line in line.lines() {
                        if sse_line.starts_with("data: ") {
                            let data = &sse_line[6..]; // Skip "data: " prefix

                            // Check for [DONE] signal
                            if data.trim() == "[DONE]" {
                                info!("🏁 Received [DONE] signal for summary");
                                break;
                            }

                            // Parse JSON chunk
                            match serde_json::from_str::<StreamChatChunk>(data) {
                                Ok(parsed_chunk) => {
                                    // Extract content delta
                                    if let Some(choice) = parsed_chunk.choices.first() {
                                        if let Some(content) = &choice.delta.content {
                                            if !content.is_empty() {
                                                // Calculate TTFT on first token
                                                if !first_token_received {
                                                    first_token_received = true;
                                                    ttft_us = Some(request_start_time.elapsed().as_micros() as u64);
                                                    info!("⏱️ First token received for summary: {:.2}ms",
                                                          ttft_us.unwrap() as f64 / 1000.0);
                                                }

                                                // Accumulate content internally
                                                accumulated_content.push_str(content);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to parse streaming chunk: {} | Data: {}", e, data);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("Stream error: {}", e));
            }
        }
    }

    // Calculate total time
    let total_time_us = request_start_time.elapsed().as_micros() as u64;

    // Log unified TTFT metrics
    log_ttft_metrics(ttft_us, total_time_us);

    info!("✅ Summary streaming completed: {} chars, TTFT: {:?}μs",
          accumulated_content.len(), ttft_us);

    Ok(SummaryResult {
        content: accumulated_content,
        ttft_us,
        total_time_us,
    })
}

/// Internal: Streaming handler for Claude (reuses Claude streaming logic)
async fn _generate_summary_streaming_claude(
    client: &Client,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<SummaryResult, String> {
    use std::time::Instant;
    let request_start_time = Instant::now();

    // Build headers
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "x-api-key",
        api_key
            .parse()
            .map_err(|_| "Invalid API key format".to_string())?,
    );
    headers.insert(
        "anthropic-version",
        "2023-06-01"
            .parse()
            .map_err(|_| "Invalid anthropic version".to_string())?,
    );
    headers.insert(
        header::CONTENT_TYPE,
        "application/json"
            .parse()
            .map_err(|_| "Invalid content type".to_string())?,
    );

    // Build streaming request body
    let request_body = serde_json::json!({
        "model": model_name,
        "max_tokens": 2048,
        "messages": vec![ChatMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        }],
        "system": system_prompt,
        "stream": true,
    });

    let api_url = "https://api.anthropic.com/v1/messages";
    info!("🚀 Sending streaming summary request to Claude: {}", api_url);

    // Send request
    let response = client
        .post(api_url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to send streaming request to Claude: {}", e))?;

    if !response.status().is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Claude streaming API request failed: {}", error_body));
    }

    // Process streaming response - accumulate tokens internally
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut accumulated_content = String::new();
    let mut ttft_us: Option<u64> = None;
    let mut first_token_received = false;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                let chunk_str = String::from_utf8_lossy(&chunk);
                buffer.push_str(&chunk_str);

                // Process complete lines (SSE format: "event: ...\ndata: {...}\n\n")
                while let Some(line_end) = buffer.find("\n\n") {
                    let block = buffer[..line_end].to_string();
                    buffer = buffer[line_end + 2..].to_string();

                    // Parse SSE block
                    let mut data: Option<String> = None;

                    for line in block.lines() {
                        if line.starts_with("data: ") {
                            data = Some(line[6..].to_string());
                        }
                    }

                    if let Some(data_str) = data {
                        match serde_json::from_str::<ClaudeStreamEvent>(&data_str) {
                            Ok(event) => {
                                match event {
                                    ClaudeStreamEvent::ContentBlockDelta { delta } => {
                                        // Calculate TTFT on first token
                                        if !first_token_received && !delta.text.is_empty() {
                                            first_token_received = true;
                                            ttft_us = Some(request_start_time.elapsed().as_micros() as u64);
                                            info!("⏱️ First token received for Claude summary: {:.2}ms",
                                                  ttft_us.unwrap() as f64 / 1000.0);
                                        }

                                        // Accumulate content internally
                                        accumulated_content.push_str(&delta.text);
                                    }
                                    ClaudeStreamEvent::Error { error } => {
                                        return Err(format!("Claude error: {}", error.message));
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                warn!("⚠️ Failed to parse Claude streaming event: {} | Data: {}", e, data_str);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("Claude stream error: {}", e));
            }
        }
    }

    // Calculate total time
    let total_time_us = request_start_time.elapsed().as_micros() as u64;

    // Log unified TTFT metrics
    log_ttft_metrics(ttft_us, total_time_us);

    info!("✅ Claude summary streaming completed: {} chars, TTFT: {:?}μs",
          accumulated_content.len(), ttft_us);

    Ok(SummaryResult {
        content: accumulated_content,
        ttft_us,
        total_time_us,
    })
}

/// Helper function to get provider name for logging
fn provider_name(provider: &LLMProvider) -> &str {
    match provider {
        LLMProvider::OpenAI => "OpenAI",
        LLMProvider::Claude => "Claude",
        LLMProvider::Groq => "Groq",
        LLMProvider::Ollama => "Ollama",
        LLMProvider::OpenRouter => "OpenRouter",
        LLMProvider::OpenAICompatible => "OpenAI Compatible",
    }
}

// ============================================================================
// STREAMING CHAT FUNCTIONS
// ============================================================================

/// Streams chat completion from LLM provider and emits Tauri events
///
/// # Arguments
/// * `app` - Tauri app handle for emitting events
/// * `client` - Reqwest HTTP client
/// * `provider` - The LLM provider to use
/// * `model_name` - The specific model to use
/// * `api_key` - API key for the provider (not needed for Ollama)
/// * `messages` - Chat history (system + user messages)
/// * `request_id` - Unique identifier for this request (for event routing)
/// * `ollama_endpoint` - Optional custom Ollama endpoint
/// * `openai_compatible_endpoint` - Optional custom OpenAI-compatible endpoint
/// * `temperature` - Optional sampling temperature (0.0 to 2.0)
/// * `top_p` - Optional nucleus sampling parameter
/// * `max_tokens` - Optional maximum tokens to generate
///
/// # Events Emitted
/// * `llm:chat:token` - Each token/chunk received
/// * `llm:chat:done` - Completion with usage stats
/// * `llm:chat:error` - Error during streaming
pub async fn stream_chat<R: Runtime>(
    app: tauri::AppHandle<R>,
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    messages: Vec<ChatMessage>,
    request_id: String,
    ollama_endpoint: Option<&str>,
    openai_compatible_endpoint: Option<&str>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<(), String> {
    info!(
        "🌊 Starting streaming chat for request_id: {} with provider: {:?}, model: {}",
        request_id, provider, model_name
    );

    let result = match provider {
        LLMProvider::Claude => {
            stream_chat_claude(
                app.clone(),
                client,
                model_name,
                api_key,
                messages,
                request_id.clone(),
                temperature,
                top_p,
                max_tokens,
            )
            .await
        }
        _ => {
            // OpenAI, Groq, OpenRouter, Ollama, OpenAI-compatible all use OpenAI-compatible format
            stream_chat_openai_compatible(
                app.clone(),
                client,
                provider,
                model_name,
                api_key,
                messages,
                request_id.clone(),
                ollama_endpoint,
                openai_compatible_endpoint,
                temperature,
                top_p,
                max_tokens,
            )
            .await
        }
    };

    match result {
        Ok(_) => {
            info!("✅ Streaming completed successfully for request_id: {}", request_id);
            Ok(())
        }
        Err(e) => {
            error!("❌ Streaming failed for request_id: {}: {}", request_id, e);
            // Emit error event
            let _ = app.emit(
                "llm:chat:error",
                StreamErrorPayload {
                    request_id: request_id.clone(),
                    message: e.clone(),
                },
            );
            Err(e)
        }
    }
}

/// Streams chat from OpenAI-compatible providers (OpenAI, Groq, OpenRouter, Ollama, OpenAI-compatible)
async fn stream_chat_openai_compatible<R: Runtime>(
    app: tauri::AppHandle<R>,
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    messages: Vec<ChatMessage>,
    request_id: String,
    ollama_endpoint: Option<&str>,
    openai_compatible_endpoint: Option<&str>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<(), String> {
    // Determine API endpoint
    let (api_url, mut headers) = match provider {
        LLMProvider::OpenAI => (
            "https://api.openai.com/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Groq => (
            "https://api.groq.com/openai/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::OpenRouter => (
            "https://openrouter.ai/api/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Ollama => {
            let host = ollama_endpoint
                .map(|s| s.to_string())
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            (
                format!("{}/v1/chat/completions", host),
                header::HeaderMap::new(),
            )
        }
        LLMProvider::OpenAICompatible => {
            let base_url = openai_compatible_endpoint
                .ok_or("OpenAI Compatible endpoint not configured")?
                .trim_end_matches('/');
            (
                format!("{}/chat/completions", base_url),
                header::HeaderMap::new(),
            )
        }
        _ => return Err("Unsupported provider for OpenAI-compatible streaming".to_string()),
    };

    // Add authorization header
    if !api_key.is_empty() || provider != &LLMProvider::OpenAICompatible {
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", api_key)
                .parse()
                .map_err(|_| "Invalid authorization header".to_string())?,
        );
    }
    headers.insert(
        header::CONTENT_TYPE,
        "application/json"
            .parse()
            .map_err(|_| "Invalid content type".to_string())?,
    );

    // Build streaming request body
    let request_body = StreamingChatRequest {
        model: model_name.to_string(),
        messages,
        stream: true,
        temperature,
        top_p,
        max_tokens,
    };

    info!("🚀 Sending streaming request to {}: {}", provider_name(provider), api_url);

    // Send request
    let response = client
        .post(&api_url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to send streaming request: {}", e))?;

    if !response.status().is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Streaming API request failed: {}", error_body));
    }

    // Process streaming response
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut finish_reason: Option<String> = None;
    let mut usage_stats: Option<StreamUsage> = None;
    let mut ttft_us: Option<u64> = None;
    let request_start_time = std::time::Instant::now();
    let mut first_token_received = false;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                // Convert bytes to string
                let chunk_str = String::from_utf8_lossy(&chunk);
                buffer.push_str(&chunk_str);

                // Process complete lines (SSE format: "data: {...}\n\n")
                while let Some(line_end) = buffer.find("\n\n") {
                    let line = buffer[..line_end].to_string();
                    buffer = buffer[line_end + 2..].to_string();

                    // Parse SSE line
                    for sse_line in line.lines() {
                        if sse_line.starts_with("data: ") {
                            let data = &sse_line[6..]; // Skip "data: " prefix

                            // Check for [DONE] signal
                            if data.trim() == "[DONE]" {
                                info!("🏁 Received [DONE] signal");
                                break;
                            }

                            // Parse JSON chunk
                            match serde_json::from_str::<StreamChatChunk>(data) {
                                Ok(parsed_chunk) => {
                                    // Extract content delta
                                    if let Some(choice) = parsed_chunk.choices.first() {
                                        if let Some(content) = &choice.delta.content {
                                            if !content.is_empty() {
                                                // Calculate TTFT on first token
                                                if !first_token_received {
                                                    first_token_received = true;
                                                    // Calculate TTFT in microseconds for higher precision
                                                    ttft_us = Some(request_start_time.elapsed().as_micros() as u64);
                                                }

                                                // Emit token event
                                                let _ = app.emit(
                                                    "llm:chat:token",
                                                    StreamTokenPayload {
                                                        request_id: request_id.clone(),
                                                        content_delta: content.clone(),
                                                    },
                                                );
                                            }
                                        }

                                        // Capture finish reason
                                        if let Some(reason) = &choice.finish_reason {
                                            finish_reason = Some(reason.clone());
                                        }
                                    }

                                    // Capture usage stats
                                    if let Some(usage) = parsed_chunk.usage {
                                        usage_stats = Some(StreamUsage {
                                            prompt_tokens: usage.prompt_tokens,
                                            completion_tokens: usage.completion_tokens,
                                            total_tokens: usage.total_tokens,
                                        });
                                    }
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to parse streaming chunk: {} | Data: {}", e, data);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("Stream error: {}", e));
            }
        }
    }

    // Log unified TTFT metrics at completion
    let total_time_us = request_start_time.elapsed().as_micros() as u64;
    log_ttft_metrics(ttft_us, total_time_us);

    // Emit completion event
    let _ = app.emit(
        "llm:chat:done",
        StreamDonePayload {
            request_id: request_id.clone(),
            usage: usage_stats,
            finish_reason,
            model: Some(model_name.to_string()),
            provider: Some(provider_name(provider).to_string()),
            ttft_us,
        },
    );

    Ok(())
}

/// Streams chat from Claude (Anthropic)
async fn stream_chat_claude<R: Runtime>(
    app: tauri::AppHandle<R>,
    client: &Client,
    model_name: &str,
    api_key: &str,
    messages: Vec<ChatMessage>,
    request_id: String,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<(), String> {
    // Separate system message from user messages
    let system_prompt = messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let user_messages: Vec<ChatMessage> = messages
        .into_iter()
        .filter(|m| m.role != "system")
        .collect();

    // Build headers
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "x-api-key",
        api_key
            .parse()
            .map_err(|_| "Invalid API key format".to_string())?,
    );
    headers.insert(
        "anthropic-version",
        "2023-06-01"
            .parse()
            .map_err(|_| "Invalid anthropic version".to_string())?,
    );
    headers.insert(
        header::CONTENT_TYPE,
        "application/json"
            .parse()
            .map_err(|_| "Invalid content type".to_string())?,
    );

    // Build streaming request body
    let mut request_body = serde_json::json!({
        "model": model_name,
        "max_tokens": max_tokens.unwrap_or(2048),
        "messages": user_messages,
        "stream": true,
    });

    if !system_prompt.is_empty() {
        request_body["system"] = serde_json::json!(system_prompt);
    }
    if let Some(temp) = temperature {
        request_body["temperature"] = serde_json::json!(temp);
    }
    if let Some(p) = top_p {
        request_body["top_p"] = serde_json::json!(p);
    }

    let api_url = "https://api.anthropic.com/v1/messages";
    info!("🚀 Sending streaming request to Claude: {}", api_url);

    // Send request
    let response = client
        .post(api_url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to send streaming request to Claude: {}", e))?;

    if !response.status().is_success() {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Claude streaming API request failed: {}", error_body));
    }

    // Process streaming response
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut finish_reason: Option<String> = None;
    let mut usage_input: Option<u32> = None;
    let mut usage_output: Option<u32> = None;
    let mut ttft_us: Option<u64> = None;
    let request_start_time = std::time::Instant::now();
    let mut first_token_received = false;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                let chunk_str = String::from_utf8_lossy(&chunk);
                buffer.push_str(&chunk_str);

                // Process complete lines (SSE format: "event: ...\ndata: {...}\n\n")
                while let Some(line_end) = buffer.find("\n\n") {
                    let block = buffer[..line_end].to_string();
                    buffer = buffer[line_end + 2..].to_string();

                    // Parse SSE block
                    let mut event_type: Option<String> = None;
                    let mut data: Option<String> = None;

                    for line in block.lines() {
                        if line.starts_with("event: ") {
                            event_type = Some(line[7..].to_string());
                        } else if line.starts_with("data: ") {
                            data = Some(line[6..].to_string());
                        }
                    }

                    if let Some(data_str) = data {
                        match serde_json::from_str::<ClaudeStreamEvent>(&data_str) {
                            Ok(event) => {
                                match event {
                                    ClaudeStreamEvent::MessageStart { message } => {
                                        info!("📝 Claude message started: {}", message.id);
                                        if let Some(input) = message.usage.input_tokens {
                                            usage_input = Some(input);
                                        }
                                    }
                                    ClaudeStreamEvent::ContentBlockDelta { delta } => {
                                        // Calculate TTFT on first token
                                        if !first_token_received && !delta.text.is_empty() {
                                            first_token_received = true;
                                            ttft_us = Some(request_start_time.elapsed().as_micros() as u64);
                                        }

                                        // Emit token event
                                        let _ = app.emit(
                                            "llm:chat:token",
                                            StreamTokenPayload {
                                                request_id: request_id.clone(),
                                                content_delta: delta.text.clone(),
                                            },
                                        );
                                    }
                                    ClaudeStreamEvent::MessageDelta { delta, usage } => {
                                        if let Some(reason) = delta.stop_reason {
                                            finish_reason = Some(reason);
                                        }
                                        if let Some(output) = usage.output_tokens {
                                            usage_output = Some(output);
                                        }
                                    }
                                    ClaudeStreamEvent::Error { error } => {
                                        return Err(format!("Claude error: {}", error.message));
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                warn!("⚠️ Failed to parse Claude streaming event: {} | Data: {}", e, data_str);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("Claude stream error: {}", e));
            }
        }
    }

    // Emit completion event
    let usage = if usage_input.is_some() || usage_output.is_some() {
        Some(StreamUsage {
            prompt_tokens: usage_input,
            completion_tokens: usage_output,
            total_tokens: match (usage_input, usage_output) {
                (Some(i), Some(o)) => Some(i + o),
                _ => None,
            },
        })
    } else {
        None
    };

    // Log unified TTFT metrics at completion
    let total_time_us = request_start_time.elapsed().as_micros() as u64;
    log_ttft_metrics(ttft_us, total_time_us);

    let _ = app.emit(
        "llm:chat:done",
        StreamDonePayload {
            request_id: request_id.clone(),
            usage,
            finish_reason,
            model: Some(model_name.to_string()),
            provider: Some("Claude".to_string()),
            ttft_us,
        },
    );

    Ok(())
}
