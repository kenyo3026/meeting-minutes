/// Chat module - handles real-time chat with LLM about meeting transcripts
///
/// This module provides:
/// - Commands for sending chat messages with streaming responses
/// - Commands for retrieving meeting context
/// - Integration with existing LLM providers
/// - System prompt construction and message processing

pub mod commands;
pub mod processor;

// Re-export Tauri commands
pub use commands::{
    __cmd__chat_send_message,
    __cmd__chat_get_meeting_context,
    chat_send_message,
    chat_get_meeting_context,
};


