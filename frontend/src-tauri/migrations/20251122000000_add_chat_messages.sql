-- Create chat_messages table for persistent chat history
CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    ttft_us INTEGER,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

-- Create index for faster queries by meeting_id
CREATE INDEX IF NOT EXISTS idx_chat_messages_meeting_id ON chat_messages(meeting_id);

-- Create index for ordering by creation time
CREATE INDEX IF NOT EXISTS idx_chat_messages_created_at ON chat_messages(meeting_id, created_at);

