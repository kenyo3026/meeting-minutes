-- Add OpenAI Compatible endpoint and API key columns to settings table
ALTER TABLE settings ADD COLUMN openaiCompatibleEndpoint TEXT;
ALTER TABLE settings ADD COLUMN openaiCompatibleApiKey TEXT;

