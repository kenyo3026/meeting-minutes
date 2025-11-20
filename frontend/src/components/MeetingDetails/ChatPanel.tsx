"use client";

import { useState, useEffect, useRef } from 'react';
import { MessageSquare, Send, FileText, ChevronDown, ChevronUp, Maximize2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { Summary, Transcript } from '@/types';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { toast } from 'sonner';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { BlockNoteSummaryView } from '@/components/AISummary/BlockNoteSummaryView';
import { SummaryGeneratorButtonGroup } from './SummaryGeneratorButtonGroup';
import { SummaryUpdaterButtonGroup } from './SummaryUpdaterButtonGroup';
import Analytics from '@/lib/analytics';
import dynamic from 'next/dynamic';
import { useCreateBlockNote } from '@blocknote/react';
import { BlockNoteView } from '@blocknote/shadcn';
import "@blocknote/shadcn/style.css";

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
  // Props needed for the full summary view dialog
  summaryPanelProps?: {
    meetingTitle: string;
    onTitleChange: (title: string) => void;
    isEditingTitle: boolean;
    onStartEditTitle: () => void;
    onFinishEditTitle: () => void;
    isTitleDirty: boolean;
    summaryRef: any;
    isSaving: boolean;
    onSaveAll: () => Promise<void>;
    onCopySummary: () => Promise<void>;
    onOpenFolder: () => Promise<void>;
    transcripts: Transcript[];
    onGenerateSummary: (customPrompt: string) => Promise<void>;
    customPrompt: string;
    summaryResponse: any;
    onSaveSummary: (summary: Summary | { markdown?: string; summary_json?: any[] }) => Promise<void>;
    onSummaryChange: (summary: Summary) => void;
    onDirtyChange: (isDirty: boolean) => void;
    summaryError: string | null;
    onRegenerateSummary: () => Promise<void>;
    getSummaryStatusMessage: (status: 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error') => string;
    availableTemplates: Array<{id: string, name: string, description: string}>;
    selectedTemplate: string;
    selectedLanguage: string;
    onTemplateSelect: (templateId: string, templateName: string) => void;
    onLanguageSelect: (languageCode: string) => void;
  };
}

interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  ttft_us?: number; // Time To First Token in microseconds (1 ms = 1000 μs)
}

interface MeetingContext {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  transcript: string;
}

export function ChatPanel({
  meeting,
  modelConfig,
  setModelConfig,
  onSaveModelConfig,
  isModelConfigLoading = false,
  aiSummary,
  summaryStatus,
  summaryPanelProps
}: ChatPanelProps) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [context, setContext] = useState<MeetingContext | null>(null);
  const [isNoteExpanded, setIsNoteExpanded] = useState(true);
  const [isFullSummaryOpen, setIsFullSummaryOpen] = useState(false);
  const streamingContentRef = useRef('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const shouldAutoScrollRef = useRef(true);
  const lastScrollTimeRef = useRef(0);

  // Create a read-only BlockNote editor for preview
  const previewEditor = useCreateBlockNote({
    initialContent: undefined
  });

  // Update preview editor when aiSummary changes
  useEffect(() => {
    const updatePreview = async () => {
      if (!aiSummary || !previewEditor) return;

      try {
        // Check if we have BlockNote JSON format
        if ((aiSummary as any).summary_json) {
          previewEditor.replaceBlocks(previewEditor.document, (aiSummary as any).summary_json);
        }
        // Otherwise, try parsing markdown
        else if ((aiSummary as any).markdown) {
          const blocks = await previewEditor.tryParseMarkdownToBlocks((aiSummary as any).markdown);
          previewEditor.replaceBlocks(previewEditor.document, blocks);
        }
      } catch (err) {
        console.error('Failed to update preview editor:', err);
      }
    };

    updatePreview();
  }, [aiSummary, previewEditor]);

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
        const { request_id, ttft_us } = event.payload;
        if (request_id === meeting.id) {
          // Update the last assistant message with TTFT
          if (ttft_us !== undefined) {
            setMessages(prev => {
              const newMessages = [...prev];
              if (newMessages.length > 0 && newMessages[newMessages.length - 1].role === 'assistant') {
                newMessages[newMessages.length - 1] = {
                  ...newMessages[newMessages.length - 1],
                  ttft_us: ttft_us
                };
              }
              return newMessages;
            });
          }
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

  // Format TTFT for display (input is in microseconds)
  const formatTTFT = (ttft_us: number): string => {
    // Convert microseconds to milliseconds with decimal precision
    const ttft_ms = ttft_us / 1000;

    if (ttft_ms < 1000) {
      // Less than 1 second - show in milliseconds with 2 decimal places
      return `${ttft_ms.toFixed(2)}ms`;
    } else if (ttft_ms < 60000) {
      // Less than 1 minute - show in seconds with 2 decimal places
      const seconds = (ttft_ms / 1000).toFixed(2);
      return `${seconds}s`;
    } else {
      // 1 minute or more - show in mm:ss.ms format
      const minutes = Math.floor(ttft_ms / 60000);
      const remainingMs = ttft_ms % 60000;
      const seconds = Math.floor(remainingMs / 1000);
      const ms = (remainingMs % 1000).toFixed(2);
      return `${minutes}:${seconds.toString().padStart(2, '0')}.${ms}`;
    }
  };

  return (
    <>
      {/* Full Summary View Dialog */}
      {summaryPanelProps && (
        <Dialog open={isFullSummaryOpen} onOpenChange={setIsFullSummaryOpen}>
          <DialogContent className="max-w-[95vw] w-[95vw] h-[95vh] p-0 overflow-hidden flex flex-col">
            <DialogHeader className="px-6 py-4 border-b border-gray-200 flex-shrink-0">
              <DialogTitle>Meeting Summary - Full View</DialogTitle>
              <DialogDescription>
                Complete summary view with all editing capabilities
              </DialogDescription>
            </DialogHeader>

            {/* Embed the complete Summary Panel content */}
            <div className="flex-1 flex flex-col overflow-hidden">
              {/* Title area and button groups */}
              <div className="relative flex-shrink-0 p-4 border-b border-gray-200">
                {/* Button groups - only show when summary exists */}
                {aiSummary && !(summaryStatus === 'processing' || summaryStatus === 'summarizing' || summaryStatus === 'regenerating') && (
                  <div className="flex items-center justify-center w-full gap-2">
                    {/* Left-aligned: Summary Generator Button Group */}
                    <div className="flex-shrink-0">
                      <SummaryGeneratorButtonGroup
                        modelConfig={modelConfig}
                        setModelConfig={setModelConfig}
                        onSaveModelConfig={onSaveModelConfig}
                        onGenerateSummary={summaryPanelProps.onGenerateSummary}
                        customPrompt={summaryPanelProps.customPrompt}
                        summaryStatus={summaryStatus}
                        availableTemplates={summaryPanelProps.availableTemplates}
                        selectedTemplate={summaryPanelProps.selectedTemplate}
                        selectedLanguage={summaryPanelProps.selectedLanguage}
                        onTemplateSelect={summaryPanelProps.onTemplateSelect}
                        onLanguageSelect={summaryPanelProps.onLanguageSelect}
                        hasTranscripts={summaryPanelProps.transcripts.length > 0}
                        isModelConfigLoading={isModelConfigLoading}
                        // onChatClick={() => {
                        //     setIsFullSummaryOpen(false);
                        //   }}
                        />
                    </div>

                    {/* Right-aligned: Summary Updater Button Group */}
                    <div className="flex-shrink-0">
                      <SummaryUpdaterButtonGroup
                        isSaving={summaryPanelProps.isSaving}
                        isDirty={summaryPanelProps.isTitleDirty || (summaryPanelProps.summaryRef.current?.isDirty || false)}
                        onSave={summaryPanelProps.onSaveAll}
                        onCopy={summaryPanelProps.onCopySummary}
                        onFind={() => {
                          console.log('Find in summary clicked');
                        }}
                        onOpenFolder={summaryPanelProps.onOpenFolder}
                        hasSummary={!!aiSummary}
                      />
                    </div>
                  </div>
                )}

                {/* Timing Metrics Display - positioned in top-right of modal */}
                {aiSummary && !(summaryStatus === 'processing' || summaryStatus === 'summarizing' || summaryStatus === 'regenerating') && (() => {
                  const ttft = (aiSummary as any)?.ttft_us;
                  const totalTime = (aiSummary as any)?.total_time_us;
                  // Always show timing metrics if summary exists (even if ttft is None)
                  if (totalTime !== undefined) {
                    return (
                      <div className="absolute top-2 right-6 flex items-center gap-3">
                        <span className="text-[10px] text-gray-500 opacity-70">
                          ttft: {ttft !== undefined && ttft !== null ? formatTTFT(ttft) : 'N/A'}
                        </span>
                        <span className="text-[10px] text-gray-500 opacity-70">
                          total: {formatTTFT(totalTime)}
                        </span>
                      </div>
                    );
                  }
                  return null;
                })()}
              </div>

              {/* Loading state */}
              {(summaryStatus === 'processing' || summaryStatus === 'summarizing' || summaryStatus === 'regenerating') ? (
                <div className="flex flex-col h-full">
                  <div className="flex items-center justify-center flex-1">
                    <div className="text-center">
                      <div className="inline-block animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-blue-500 mb-4"></div>
                      <p className="text-gray-600">Generating AI Summary...</p>
                    </div>
                  </div>
                </div>
              ) : aiSummary && summaryPanelProps.transcripts?.length > 0 ? (
                /* Summary editor area */
                <div className="flex-1 overflow-y-auto min-h-0">
                  <div className="p-6 w-full">
                    <BlockNoteSummaryView
                      ref={summaryPanelProps.summaryRef}
                      summaryData={aiSummary}
                      onSave={summaryPanelProps.onSaveSummary}
                      onSummaryChange={summaryPanelProps.onSummaryChange}
                      onDirtyChange={summaryPanelProps.onDirtyChange}
                      status={summaryStatus}
                      error={summaryPanelProps.summaryError}
                      onRegenerateSummary={() => {
                        Analytics.trackButtonClick('regenerate_summary', 'meeting_details');
                        summaryPanelProps.onRegenerateSummary();
                      }}
                      meeting={{
                        id: meeting.id,
                        title: summaryPanelProps.meetingTitle,
                        created_at: meeting.created_at
                      }}
                    />
                  </div>
                  {summaryStatus !== 'idle' && (
                    <div className={`mt-4 mx-6 p-4 rounded-lg ${summaryStatus === 'error' ? 'bg-red-100 text-red-700' :
                      summaryStatus === 'completed' ? 'bg-green-100 text-green-700' :
                        'bg-blue-100 text-blue-700'
                      }`}>
                      <p className="text-sm font-medium">{summaryPanelProps.getSummaryStatusMessage(summaryStatus)}</p>
                    </div>
                  )}
                </div>
              ) : (
                <div className="flex items-center justify-center flex-1">
                  <p className="text-gray-500">No summary available</p>
                </div>
              )}
            </div>
          </DialogContent>
        </Dialog>
      )}

      {/* Main Chat Panel UI */}
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

      {/* Note Preview Section - Collapsible */}
      <div
        className="flex-shrink-0 border-b border-gray-200 bg-gray-50 transition-all duration-300 ease-in-out"
        style={{ height: isNoteExpanded ? '25%' : 'auto' }}
      >
        {/* Header - always visible */}
        <div className="flex items-center justify-between px-4 py-2 border-b border-gray-200 bg-white">
          <div className="flex items-center gap-2">
            <FileText className="h-4 w-4 text-gray-600" />
            <h3 className="text-sm font-semibold text-gray-700">Meeting Note Preview</h3>
            <span className="text-xs text-gray-500">
              {summaryStatus === 'processing' || summaryStatus === 'summarizing'
                ? '🔄 Generating...'
                : summaryStatus === 'completed' && aiSummary
                ? '✓ Synced'
                : ''}
            </span>
            {/* Timing Metrics */}
            {aiSummary && (() => {
              const ttft = (aiSummary as any)?.ttft_us;
              const totalTime = (aiSummary as any)?.total_time_us;
              // Always show timing metrics if summary exists (even if ttft is None)
              if (totalTime !== undefined) {
                return (
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-gray-400">•</span>
                    <span className="text-[10px] text-gray-500 opacity-70">
                      ttft: {ttft !== undefined && ttft !== null ? formatTTFT(ttft) : 'N/A'}
                    </span>
                    <span className="text-[10px] text-gray-500 opacity-70">
                      total: {formatTTFT(totalTime)}
                    </span>
                  </div>
                );
              }
              return null;
            })()}
          </div>
          <div className="flex items-center gap-1">
            {/* Expand to full view button */}
            {aiSummary && summaryPanelProps && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setIsFullSummaryOpen(true)}
                className="h-7 px-2"
                title="Open full summary view"
              >
                <Maximize2 className="h-4 w-4" />
              </Button>
            )}
            {/* Collapse/expand preview button */}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setIsNoteExpanded(!isNoteExpanded)}
              className="h-7 px-2"
              title={isNoteExpanded ? 'Collapse note preview' : 'Expand note preview'}
            >
              {isNoteExpanded ? (
                <ChevronUp className="h-4 w-4" />
              ) : (
                <ChevronDown className="h-4 w-4" />
              )}
            </Button>
          </div>
        </div>

        {/* Content - collapsible and clickable */}
        {isNoteExpanded && (
          <div
            className={`h-full overflow-y-auto p-4 ${aiSummary && summaryPanelProps ? 'cursor-pointer hover:bg-gray-100 transition-colors' : ''}`}
            style={{ height: 'calc(100% - 44px)' }}
            onClick={() => {
              if (aiSummary && summaryPanelProps) {
                setIsFullSummaryOpen(true);
              }
            }}
            title={aiSummary && summaryPanelProps ? 'Click to open full summary view' : ''}
          >
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
              <div className="w-full blocknote-preview-readonly">
                <BlockNoteView
                  editor={previewEditor}
                  editable={false}
                  theme="light"
                />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Chat Messages Section - Flexible height */}
      <div className="flex flex-col flex-1 min-h-0">
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
                  <div className={`text-xs mt-1 flex items-center gap-2 ${
                    message.role === 'user' ? 'text-blue-100' : 'text-gray-500'
                  }`}>
                    <span>{message.timestamp.toLocaleTimeString()}</span>
                    {message.role === 'assistant' && message.ttft_us !== undefined && (
                      <span className="text-[10px] opacity-70">
                        • ttft: {formatTTFT(message.ttft_us)}
                      </span>
                    )}
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
    </>
  );
}

