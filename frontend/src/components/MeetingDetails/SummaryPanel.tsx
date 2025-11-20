"use client";

import { Summary, SummaryResponse, Transcript } from '@/types';
import { EditableTitle } from '@/components/EditableTitle';
import { BlockNoteSummaryView, BlockNoteSummaryViewRef } from '@/components/AISummary/BlockNoteSummaryView';
import { EmptyStateSummary } from '@/components/EmptyStateSummary';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { SummaryGeneratorButtonGroup } from './SummaryGeneratorButtonGroup';
import { SummaryUpdaterButtonGroup } from './SummaryUpdaterButtonGroup';
import { ChatPanel } from './ChatPanel';
import Analytics from '@/lib/analytics';
import { RefObject, useState } from 'react';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface SummaryPanelProps {
  meeting: {
    id: string;
    title: string;
    created_at: string;
  };
  meetingTitle: string;
  onTitleChange: (title: string) => void;
  isEditingTitle: boolean;
  onStartEditTitle: () => void;
  onFinishEditTitle: () => void;
  isTitleDirty: boolean;
  summaryRef: RefObject<BlockNoteSummaryViewRef>;
  isSaving: boolean;
  onSaveAll: () => Promise<void>;
  onCopySummary: () => Promise<void>;
  onOpenFolder: () => Promise<void>;
  aiSummary: Summary | null;
  summaryStatus: 'idle' | 'processing' | 'summarizing' | 'regenerating' | 'completed' | 'error';
  transcripts: Transcript[];
  modelConfig: ModelConfig;
  setModelConfig: (config: ModelConfig | ((prev: ModelConfig) => ModelConfig)) => void;
  onSaveModelConfig: (config?: ModelConfig) => Promise<void>;
  onGenerateSummary: (customPrompt: string) => Promise<void>;
  customPrompt: string;
  summaryResponse: SummaryResponse | null;
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
  isModelConfigLoading?: boolean;
}

// Helper function to format time (microseconds to human-readable)
const formatTime = (time_us: number): string => {
  const time_ms = time_us / 1000;

  if (time_ms < 1000) {
    return `${time_ms.toFixed(2)}ms`;
  } else if (time_ms < 60000) {
    const seconds = (time_ms / 1000).toFixed(2);
    return `${seconds}s`;
  } else {
    const minutes = Math.floor(time_ms / 60000);
    const remainingMs = time_ms % 60000;
    const seconds = Math.floor(remainingMs / 1000);
    const ms = (remainingMs % 1000).toFixed(0);
    return `${minutes}:${seconds.toString().padStart(2, '0')}.${ms}`;
  }
};

export function SummaryPanel({
  meeting,
  meetingTitle,
  onTitleChange,
  isEditingTitle,
  onStartEditTitle,
  onFinishEditTitle,
  isTitleDirty,
  summaryRef,
  isSaving,
  onSaveAll,
  onCopySummary,
  onOpenFolder,
  aiSummary,
  summaryStatus,
  transcripts,
  modelConfig,
  setModelConfig,
  onSaveModelConfig,
  onGenerateSummary,
  customPrompt,
  summaryResponse,
  onSaveSummary,
  onSummaryChange,
  onDirtyChange,
  summaryError,
  onRegenerateSummary,
  getSummaryStatusMessage,
  availableTemplates,
  selectedTemplate,
  selectedLanguage,
  onTemplateSelect,
  onLanguageSelect,
  isModelConfigLoading = false
}: SummaryPanelProps) {
  const isSummaryLoading = summaryStatus === 'processing' || summaryStatus === 'summarizing' || summaryStatus === 'regenerating';
  const [showChat, setShowChat] = useState(false);

  // If chat is shown, render ChatPanel
  if (showChat) {
    return (
      <div className="flex-1 min-w-0 flex flex-col bg-white overflow-hidden">
        {/* Back button */}
        <div className="p-4 border-b border-gray-200">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowChat(false)}
            className="gap-2"
          >
            <ArrowLeft className="h-4 w-4" />
            Back to Summary
          </Button>
        </div>

        {/* Chat Panel */}
        <div className="flex-1 overflow-hidden">
          <ChatPanel
            meeting={meeting}
            modelConfig={modelConfig}
            setModelConfig={setModelConfig}
            onSaveModelConfig={onSaveModelConfig}
            isModelConfigLoading={isModelConfigLoading}
            aiSummary={aiSummary}
            summaryStatus={summaryStatus}
            summaryPanelProps={{
              meetingTitle,
              onTitleChange,
              isEditingTitle,
              onStartEditTitle,
              onFinishEditTitle,
              isTitleDirty,
              summaryRef,
              isSaving,
              onSaveAll,
              onCopySummary,
              onOpenFolder,
              transcripts,
              onGenerateSummary,
              customPrompt,
              summaryResponse,
              onSaveSummary,
              onSummaryChange,
              onDirtyChange,
              summaryError,
              onRegenerateSummary,
              getSummaryStatusMessage,
              availableTemplates,
              selectedTemplate,
              selectedLanguage,
              onTemplateSelect,
              onLanguageSelect,
            }}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 min-w-0 flex flex-col bg-white overflow-hidden">
      {/* Title area */}
      <div className="relative p-4 border-b border-gray-200">
        {/* <EditableTitle
          title={meetingTitle}
          isEditing={isEditingTitle}
          onStartEditing={onStartEditTitle}
          onFinishEditing={onFinishEditTitle}
          onChange={onTitleChange}
        /> */}

        {/* Button groups - only show when summary exists */}
        {aiSummary && !isSummaryLoading && (
          <div className="flex items-center justify-center w-full pt-0 gap-2">
            {/* Left-aligned: Summary Generator Button Group */}
            <div className="flex-shrink-0">
              <SummaryGeneratorButtonGroup
                modelConfig={modelConfig}
                setModelConfig={setModelConfig}
                onSaveModelConfig={onSaveModelConfig}
                onGenerateSummary={onGenerateSummary}
                customPrompt={customPrompt}
                summaryStatus={summaryStatus}
                availableTemplates={availableTemplates}
                selectedTemplate={selectedTemplate}
                selectedLanguage={selectedLanguage}
                onTemplateSelect={onTemplateSelect}
                onLanguageSelect={onLanguageSelect}
                hasTranscripts={transcripts.length > 0}
                isModelConfigLoading={isModelConfigLoading}
                onChatClick={() => setShowChat(true)}
              />
            </div>

            {/* Right-aligned: Summary Updater Button Group */}
            <div className="flex-shrink-0">
              <SummaryUpdaterButtonGroup
                isSaving={isSaving}
                isDirty={isTitleDirty || (summaryRef.current?.isDirty || false)}
                onSave={onSaveAll}
                onCopy={onCopySummary}
                onFind={() => {
                  // TODO: Implement find in summary functionality
                  console.log('Find in summary clicked');
                }}
                onOpenFolder={onOpenFolder}
                hasSummary={!!aiSummary}
              />
            </div>
          </div>
        )}
      </div>

      {isSummaryLoading ? (
        <div className="flex flex-col h-full">
          {/* Show button group during generation */}
          <div className="flex items-center justify-center pt-8 pb-4">
            <SummaryGeneratorButtonGroup
              modelConfig={modelConfig}
              setModelConfig={setModelConfig}
              onSaveModelConfig={onSaveModelConfig}
              onGenerateSummary={onGenerateSummary}
              customPrompt={customPrompt}
              summaryStatus={summaryStatus}
              availableTemplates={availableTemplates}
              selectedTemplate={selectedTemplate}
              selectedLanguage={selectedLanguage}
              onTemplateSelect={onTemplateSelect}
              onLanguageSelect={onLanguageSelect}
              hasTranscripts={transcripts.length > 0}
              isModelConfigLoading={isModelConfigLoading}
              onChatClick={() => setShowChat(true)}
            />
          </div>
          {/* Loading spinner */}
          <div className="flex items-center justify-center flex-1">
            <div className="text-center">
              <div className="inline-block animate-spin rounded-full h-12 w-12 border-t-2 border-b-2 border-blue-500 mb-4"></div>
              <p className="text-gray-600">Generating AI Summary...</p>
            </div>
          </div>
        </div>
      ) : !aiSummary ? (
        <div className="flex flex-col h-full">
          {/* Centered Summary Generator Button Group when no summary */}
          <div className="flex items-center justify-center pt-8 pb-4">
            <SummaryGeneratorButtonGroup
              modelConfig={modelConfig}
              setModelConfig={setModelConfig}
              onSaveModelConfig={onSaveModelConfig}
              onGenerateSummary={onGenerateSummary}
              customPrompt={customPrompt}
              summaryStatus={summaryStatus}
              availableTemplates={availableTemplates}
              selectedTemplate={selectedTemplate}
              selectedLanguage={selectedLanguage}
              onTemplateSelect={onTemplateSelect}
              onLanguageSelect={onLanguageSelect}
              hasTranscripts={transcripts.length > 0}
              isModelConfigLoading={isModelConfigLoading}
              onChatClick={() => setShowChat(true)}
            />
          </div>
          {/* Empty state message */}
          <EmptyStateSummary
            onGenerate={() => onGenerateSummary(customPrompt)}
            hasModel={modelConfig.provider !== null && modelConfig.model !== null}
            isGenerating={isSummaryLoading}
          />
        </div>
      ) : transcripts?.length > 0 && (
        <div className="flex-1 overflow-y-auto min-h-0">
          {summaryResponse && (
            <div className="fixed bottom-0 left-0 right-0 bg-white shadow-lg p-4 max-h-1/3 overflow-y-auto">
              <h3 className="text-lg font-semibold mb-2">Meeting Summary</h3>
              <div className="grid grid-cols-2 gap-4">
                <div className="bg-white p-4 rounded-lg shadow-sm">
                  <h4 className="font-medium mb-1">Key Points</h4>
                  <ul className="list-disc pl-4">
                    {summaryResponse.summary.key_points.blocks.map((block, i) => (
                      <li key={i} className="text-sm">{block.content}</li>
                    ))}
                  </ul>
                </div>
                <div className="bg-white p-4 rounded-lg shadow-sm mt-4">
                  <h4 className="font-medium mb-1">Action Items</h4>
                  <ul className="list-disc pl-4">
                    {summaryResponse.summary.action_items.blocks.map((block, i) => (
                      <li key={i} className="text-sm">{block.content}</li>
                    ))}
                  </ul>
                </div>
                <div className="bg-white p-4 rounded-lg shadow-sm mt-4">
                  <h4 className="font-medium mb-1">Decisions</h4>
                  <ul className="list-disc pl-4">
                    {summaryResponse.summary.decisions.blocks.map((block, i) => (
                      <li key={i} className="text-sm">{block.content}</li>
                    ))}
                  </ul>
                </div>
                <div className="bg-white p-4 rounded-lg shadow-sm mt-4">
                  <h4 className="font-medium mb-1">Main Topics</h4>
                  <ul className="list-disc pl-4">
                    {summaryResponse.summary.main_topics.blocks.map((block, i) => (
                      <li key={i} className="text-sm">{block.content}</li>
                    ))}
                  </ul>
                </div>
              </div>
              {summaryResponse.raw_summary ? (
                <div className="mt-4">
                  <h4 className="font-medium mb-1">Full Summary</h4>
                  <p className="text-sm whitespace-pre-wrap">{summaryResponse.raw_summary}</p>
                </div>
              ) : null}
            </div>
          )}
          {/* Timing Metrics Display - positioned above Summary content block */}
          {aiSummary && !isSummaryLoading && (() => {
            const ttft = (aiSummary as any)?.ttft_us;
            const totalTime = (aiSummary as any)?.total_time_us;
            // Always show timing metrics if summary exists (even if ttft is None)
            if (totalTime !== undefined) {
              return (
                <div className="px-6 pt-6 pb-2 flex items-center justify-end gap-3">
                  <span className="text-[10px] text-gray-500 opacity-70">
                    ttft: {ttft !== undefined && ttft !== null ? formatTime(ttft) : 'N/A'}
                  </span>
                  <span className="text-[10px] text-gray-500 opacity-70">
                    total: {formatTime(totalTime)}
                  </span>
                </div>
              );
            }
            return null;
          })()}
          <div className="p-6 w-full">
            <BlockNoteSummaryView
              ref={summaryRef}
              summaryData={aiSummary}
              onSave={onSaveSummary}
              onSummaryChange={onSummaryChange}
              onDirtyChange={onDirtyChange}
              status={summaryStatus}
              error={summaryError}
              onRegenerateSummary={() => {
                Analytics.trackButtonClick('regenerate_summary', 'meeting_details');
                onRegenerateSummary();
              }}
              meeting={{
                id: meeting.id,
                title: meetingTitle,
                created_at: meeting.created_at
              }}
            />
          </div>
          {summaryStatus !== 'idle' && (
            <div className={`mt-4 p-4 rounded-lg ${summaryStatus === 'error' ? 'bg-red-100 text-red-700' :
              summaryStatus === 'completed' ? 'bg-green-100 text-green-700' :
                'bg-blue-100 text-blue-700'
              }`}>
              <p className="text-sm font-medium">{getSummaryStatusMessage(summaryStatus)}</p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
