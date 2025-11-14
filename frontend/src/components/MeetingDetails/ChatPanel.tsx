"use client";

import { useState, useEffect, useRef } from 'react';
import { MessageSquare, Send, FileText } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { Summary } from '@/types';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import ReactMarkdown from 'react-markdown';

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
  aiSummary: Summary | null;
  summaryStatus: 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error';
}

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
}

interface MeetingContext {
  meeting_id: string;
  title: string;
  created_at: string;
  transcript: string;
  transcript_count: number;
}

export function ChatPanel({
  meeting,
  modelConfig,
  setModelConfig,
  onSaveModelConfig,
  isModelConfigLoading = false,
  aiSummary,
  summaryStatus
}: ChatPanelProps) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [context, setContext] = useState<MeetingContext | null>(null);
  const streamingContentRef = useRef('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const shouldAutoScrollRef = useRef(true);
  const lastScrollTimeRef = useRef(0);

  // Load meeting context on mount
  useEffect(() => {
    const loadContext = async () => {
      try {
        const meetingContext = await invoke<MeetingContext>('chat_get_meeting_context', {
          meetingId: meeting.id
        });
        setContext(meetingContext);
      } catch (error) {
        console.error('Failed to load meeting context:', error);
        toast.error('Failed to load meeting transcript');
      }
    };

    loadContext();
  }, [meeting.id]);

  // Set up streaming event listeners
  useEffect(() => {
    let tokenUnlisten: UnlistenFn | null = null;
    let doneUnlisten: UnlistenFn | null = null;
    let errorUnlisten: UnlistenFn | null = null;

    const setupListeners = async () => {
      // Listen for streaming tokens
      tokenUnlisten = await listen('llm:chat:token', (event: any) => {
        const { request_id, content_delta } = event.payload;
        if (request_id === meeting.id) {
          streamingContentRef.current += content_delta;

          // Update the last message with accumulated content
          setMessages(prev => {
            const newMessages = [...prev];
            if (newMessages.length > 0 && newMessages[newMessages.length - 1].role === 'assistant') {
              newMessages[newMessages.length - 1] = {
                ...newMessages[newMessages.length - 1],
                content: streamingContentRef.current
              };
            }
            return newMessages;
          });
        }
      });

      // Listen for completion
      doneUnlisten = await listen('llm:chat:done', (event: any) => {
        const { request_id } = event.payload;
        if (request_id === meeting.id) {
          setIsLoading(false);
        }
      });

      // Listen for errors
      errorUnlisten = await listen('llm:chat:error', (event: any) => {
        const { request_id, message } = event.payload;
        if (request_id === meeting.id) {
          console.error('Chat streaming error:', message);
          toast.error(`Chat error: ${message}`);
          setIsLoading(false);
          streamingContentRef.current = '';
        }
      });
    };

    setupListeners();

    return () => {
      if (tokenUnlisten) tokenUnlisten();
      if (doneUnlisten) doneUnlisten();
      if (errorUnlisten) errorUnlisten();
    };
  }, [meeting.id]);

  // Check if user has scrolled up manually
  const handleScroll = () => {
    if (!messagesContainerRef.current) return;

    const { scrollTop, scrollHeight, clientHeight } = messagesContainerRef.current;
    const distanceFromBottom = scrollHeight - scrollTop - clientHeight;

    // If user is within 100px of bottom, enable auto-scroll
    // Otherwise, disable it (user has scrolled up manually)
    shouldAutoScrollRef.current = distanceFromBottom < 100;
  };

  // Smooth auto-scroll with throttling (max once per 100ms)
  useEffect(() => {
    if (!shouldAutoScrollRef.current) return;

    const now = Date.now();
    const timeSinceLastScroll = now - lastScrollTimeRef.current;

    // Throttle scrolling to once per 100ms
    if (timeSinceLastScroll < 100) return;

    lastScrollTimeRef.current = now;

    // Use requestAnimationFrame for smooth, synced scrolling
    requestAnimationFrame(() => {
      if (messagesContainerRef.current && shouldAutoScrollRef.current) {
        const { scrollHeight, clientHeight } = messagesContainerRef.current;
        messagesContainerRef.current.scrollTop = scrollHeight - clientHeight;
      }
    });
  }, [messages]);

  const handleSend = async () => {
    if (!input.trim() || isLoading || !context) return;

    const userMessage: Message = {
      id: `user-${Date.now()}`,
      role: 'user',
      content: input.trim(),
      timestamp: new Date()
    };

    setMessages(prev => [...prev, userMessage]);
    setInput('');
    setIsLoading(true);

    // Re-enable auto-scroll when sending a new message
    shouldAutoScrollRef.current = true;

    // Create assistant message placeholder for streaming
    const assistantMessage: Message = {
      id: `assistant-${Date.now()}`,
      role: 'assistant',
      content: '',
      timestamp: new Date()
    };
    setMessages(prev => [...prev, assistantMessage]);

    // Reset streaming content
    streamingContentRef.current = '';

    try {
      // Build message history with context
      const chatMessages = [
        {
          role: 'system',
          content: `You are a helpful AI assistant analyzing a meeting transcript. Here is the meeting information:

Title: ${context.title}
Date: ${new Date(context.created_at).toLocaleString()}

Transcript:
${context.transcript}

Please answer the user's questions based on this meeting transcript.`
        },
        ...messages.map(m => ({
          role: m.role,
          content: m.content
        })),
        {
          role: 'user',
          content: userMessage.content
        }
      ];

      // Get API key (same key used for all providers)
      const apiKey = modelConfig.apiKey || undefined;

      // Get endpoint for Ollama or OpenAI-compatible
      let endpoint: string | undefined;
      if (modelConfig.provider === 'ollama') {
        endpoint = modelConfig.ollamaEndpoint || undefined;
      } else if (modelConfig.provider === 'openai-compatible') {
        endpoint = modelConfig.openaiCompatibleEndpoint || undefined;
      }

      // Send chat request
      await invoke('chat_send_message', {
        request: {
          meeting_id: meeting.id,
          messages: chatMessages,
          provider: modelConfig.provider,
          model: modelConfig.model,
          api_key: apiKey,
          endpoint: endpoint,
          temperature: 0.7,
          max_tokens: 2048
        }
      });

    } catch (error) {
      console.error('Failed to send chat message:', error);
      toast.error('Failed to send message');
      setIsLoading(false);

      // Remove the placeholder assistant message on error
      setMessages(prev => prev.slice(0, -1));
    }
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
      <div className="flex-shrink-0 px-4 py-3 border-b border-gray-200">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <MessageSquare className="h-5 w-5 text-blue-600" />
            <h2 className="text-lg font-semibold text-gray-900">Chat with AI</h2>
          </div>
          <div className="text-sm text-gray-500">
            {modelConfig.provider} • {modelConfig.model}
          </div>
        </div>
      </div>

      {/* Note Preview Section - 25% height */}
      <div className="flex-shrink-0 border-b border-gray-200 bg-gray-50" style={{ height: '25%' }}>
        <div className="h-full overflow-y-auto p-4">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <FileText className="h-4 w-4 text-gray-600" />
              <h3 className="text-sm font-semibold text-gray-700">Meeting Note Preview</h3>
            </div>
            <span className="text-xs text-gray-500">
              {summaryStatus === 'processing' || summaryStatus === 'summarizing'
                ? '🔄 Generating...'
                : summaryStatus === 'completed' && aiSummary
                ? '✓ Synced'
                : ''}
            </span>
          </div>

          {/* Summary content */}
          {!aiSummary ? (
            <div className="text-sm text-gray-600">
              <p className="mb-2">📝 No meeting notes yet.</p>
              <p className="text-xs text-gray-500">
                Generate a summary in the Summary tab to see the content here.
              </p>
            </div>
          ) : summaryStatus === 'processing' || summaryStatus === 'summarizing' ? (
            <div className="flex items-center gap-2 text-sm text-gray-600">
              <div className="w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
              <span>Generating meeting notes...</span>
            </div>
          ) : (
            <div className="prose prose-sm max-w-none text-gray-700">
              <ReactMarkdown>
                {(aiSummary as any).markdown || 'No content available'}
              </ReactMarkdown>
            </div>
          )}
        </div>
      </div>

      {/* Chat Messages Section - 75% height */}
      <div className="flex flex-col flex-1 min-h-0" style={{ maxHeight: '75%' }}>
        {/* Messages */}
        <div
          ref={messagesContainerRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto p-4 space-y-4 min-h-0"
        >
        {!context ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <div className="inline-block animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500 mb-2"></div>
              <p className="text-sm text-gray-600">Loading meeting transcript...</p>
            </div>
          </div>
        ) : messages.length === 0 ? (
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
          <>
            {messages.map((message) => (
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
                  <div className="text-sm whitespace-pre-wrap">
                    {message.content || (
                      <span className="text-gray-400 italic">Waiting for response...</span>
                    )}
                  </div>
                  <div className={`text-xs mt-1 ${
                    message.role === 'user' ? 'text-blue-100' : 'text-gray-500'
                  }`}>
                    {message.timestamp.toLocaleTimeString()}
                  </div>
                </div>
              </div>
            ))}
            <div ref={messagesEndRef} />
          </>
        )}
        {isLoading && messages.length > 0 && messages[messages.length - 1].role === 'user' && (
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
        <div className="flex-shrink-0 px-4 py-3 border-t border-gray-200 bg-white">
        <div className="flex gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask a question about your meeting..."
            className="flex-1 px-3 py-2 border border-gray-300 rounded-lg resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-sm"
            rows={2}
            disabled={isLoading}
          />
          <Button
            onClick={handleSend}
            disabled={!input.trim() || isLoading}
            className="self-end"
            size="default"
          >
            <Send className="h-4 w-4" />
          </Button>
        </div>
        <p className="text-xs text-gray-500 mt-1.5">
          Press Enter to send, Shift+Enter for new line
        </p>
        </div>
      </div>
    </div>
  );
}

