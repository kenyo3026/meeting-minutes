"use client";

import { useState } from 'react';
import { MessageSquare, Send } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ModelConfig } from '@/components/ModelSettingsModal';

interface ChatPanelProps {
  meeting: {
    id: string;
    title: string;
    created_at: string;
  };
  modelConfig: ModelConfig;
  setModelConfig: (config: ModelConfig | ((prev: ModelConfig) => ModelConfig)) => void;
  onSaveModelConfig: (config?: ModelConfig) => Promise<void>;
  isModelConfigLoading?: boolean;
}

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
}

export function ChatPanel({
  meeting,
  modelConfig,
  setModelConfig,
  onSaveModelConfig,
  isModelConfigLoading = false
}: ChatPanelProps) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const handleSend = async () => {
    if (!input.trim() || isLoading) return;

    const userMessage: Message = {
      id: `user-${Date.now()}`,
      role: 'user',
      content: input.trim(),
      timestamp: new Date()
    };

    setMessages(prev => [...prev, userMessage]);
    setInput('');
    setIsLoading(true);

    setTimeout(() => {
      const assistantMessage: Message = {
        id: `assistant-${Date.now()}`,
        role: 'assistant',
        content: 'This is a placeholder response. LLM integration will be implemented in the next step.',
        timestamp: new Date()
      };
      setMessages(prev => [...prev, assistantMessage]);
      setIsLoading(false);
    }, 1000);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="flex flex-col bg-white min-w-0 h-full">
      {/* Header */}
      <div className="flex-shrink-0 p-4 border-b border-gray-200">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <MessageSquare className="h-5 w-5 text-blue-600" />
            <h2 className="text-lg font-semibold text-gray-900">Chat with AI</h2>
          </div>
          <div className="text-sm text-gray-500">
            {modelConfig.provider} • {modelConfig.model}
          </div>
        </div>
        <p className="text-sm text-gray-600 mt-1">
          Ask questions about your meeting transcript
        </p>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4 min-h-0">
        {messages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-center">
            <div className="w-16 h-16 bg-blue-100 rounded-full flex items-center justify-center mb-4">
              <MessageSquare className="h-8 w-8 text-blue-600" />
            </div>
            <h3 className="text-lg font-medium text-gray-900 mb-2">
              Start a conversation
            </h3>
            <p className="text-sm text-gray-600 max-w-md">
              Ask questions about your meeting, get insights, or request specific information from the transcript.
            </p>
            <div className="mt-6 grid grid-cols-1 gap-2 w-full max-w-md">
              <button
                onClick={() => setInput("What were the main topics discussed?")}
                className="p-3 text-left text-sm bg-gray-50 hover:bg-gray-100 rounded-lg border border-gray-200 transition-colors"
              >
                💡 What were the main topics discussed?
              </button>
              <button
                onClick={() => setInput("Summarize the key decisions made")}
                className="p-3 text-left text-sm bg-gray-50 hover:bg-gray-100 rounded-lg border border-gray-200 transition-colors"
              >
                📋 Summarize the key decisions made
              </button>
              <button
                onClick={() => setInput("List all action items mentioned")}
                className="p-3 text-left text-sm bg-gray-50 hover:bg-gray-100 rounded-lg border border-gray-200 transition-colors"
              >
                ✅ List all action items mentioned
              </button>
            </div>
          </div>
        ) : (
          messages.map((message) => (
            <div
              key={message.id}
              className={`flex ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
            >
              <div
                className={`max-w-[80%] rounded-lg px-4 py-2 ${
                  message.role === 'user'
                    ? 'bg-blue-600 text-white'
                    : 'bg-gray-100 text-gray-900'
                }`}
              >
                <div className="text-sm whitespace-pre-wrap">{message.content}</div>
                <div className={`text-xs mt-1 ${
                  message.role === 'user' ? 'text-blue-100' : 'text-gray-500'
                }`}>
                  {message.timestamp.toLocaleTimeString()}
                </div>
              </div>
            </div>
          ))
        )}
        {isLoading && (
          <div className="flex justify-start">
            <div className="bg-gray-100 rounded-lg px-4 py-2">
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style={{ animationDelay: '0ms' }}></div>
                <div className="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style={{ animationDelay: '150ms' }}></div>
                <div className="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style={{ animationDelay: '300ms' }}></div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Input - Fixed at bottom */}
      <div className="flex-shrink-0 p-4 border-t border-gray-200 bg-white">
        <div className="flex gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask a question about your meeting..."
            className="flex-1 px-4 py-3 border border-gray-300 rounded-lg resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            rows={3}
            disabled={isLoading}
          />
          <Button
            onClick={handleSend}
            disabled={!input.trim() || isLoading}
            className="self-end"
            size="lg"
          >
            <Send className="h-5 w-5" />
          </Button>
        </div>
        <p className="text-xs text-gray-500 mt-2">
          Press Enter to send, Shift+Enter for new line
        </p>
      </div>
    </div>
  );
}

