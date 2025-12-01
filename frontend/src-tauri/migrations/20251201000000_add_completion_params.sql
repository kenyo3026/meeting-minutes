-- Add completionParams column to settings table
-- Stores JSON string of completion parameters (temperature, top_p, max_tokens, etc.)
ALTER TABLE settings ADD COLUMN completionParams TEXT;

