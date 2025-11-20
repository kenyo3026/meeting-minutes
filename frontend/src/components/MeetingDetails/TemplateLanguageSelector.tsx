"use client";

import { useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from "@/components/ui/dialog";
import { Check } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface Template {
  id: string;
  name: string;
  description: string;
}

interface Language {
  code: string;
  name: string;
}

interface TemplateLanguageSelectorProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  templates: Template[];
  selectedTemplate: string;
  onTemplateSelect: (templateId: string, templateName: string) => void;
  languages?: Language[];
  selectedLanguage?: string;
  onLanguageSelect?: (languageCode: string) => void;
}

const DEFAULT_LANGUAGES: Language[] = [
  { code: 'en', name: 'English' },
  { code: 'zh-tw', name: '繁體中文' },
  { code: 'zh-cn', name: '简体中文' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
];

export function TemplateLanguageSelector({
  open,
  onOpenChange,
  templates,
  selectedTemplate,
  onTemplateSelect,
  languages = DEFAULT_LANGUAGES,
  selectedLanguage = 'en',
  onLanguageSelect,
}: TemplateLanguageSelectorProps) {
  const [localSelectedLanguage, setLocalSelectedLanguage] = useState(selectedLanguage);

  const handleLanguageSelect = (languageCode: string) => {
    setLocalSelectedLanguage(languageCode);
    onLanguageSelect?.(languageCode);
  };

  const handleTemplateSelect = (templateId: string, templateName: string) => {
    onTemplateSelect(templateId, templateName);
    // Optionally close dialog after selection
    // onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl p-0">
        <DialogTitle className="sr-only">Select Template and Language</DialogTitle>
        <div className="flex h-[500px]">
          {/* Left Panel - Templates */}
          <div className="flex-1 border-r border-gray-200 overflow-y-auto">
            <div className="p-4 border-b border-gray-200 bg-gray-50">
              <h3 className="text-sm font-semibold text-gray-700">Template</h3>
              <p className="text-xs text-gray-500 mt-1">Select a summary template</p>
            </div>
            <div className="p-2">
              {templates.map((template) => (
                <button
                  key={template.id}
                  onClick={() => handleTemplateSelect(template.id, template.name)}
                  className={`w-full text-left px-3 py-2 rounded-md mb-1 transition-colors ${
                    selectedTemplate === template.id
                      ? 'bg-blue-50 text-blue-700 border border-blue-200'
                      : 'hover:bg-gray-50 text-gray-700'
                  }`}
                  title={template.description}
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium">{template.name}</span>
                    {selectedTemplate === template.id && (
                      <Check className="h-4 w-4 text-blue-600" />
                    )}
                  </div>
                  {template.description && (
                    <p className="text-xs text-gray-500 mt-1">{template.description}</p>
                  )}
                </button>
              ))}
            </div>
          </div>

          {/* Right Panel - Languages */}
          <div className="flex-1 overflow-y-auto">
            <div className="p-4 border-b border-gray-200 bg-gray-50">
              <h3 className="text-sm font-semibold text-gray-700">Language</h3>
              <p className="text-xs text-gray-500 mt-1">Select output language</p>
            </div>
            <div className="p-2">
              {languages.map((language) => (
                <button
                  key={language.code}
                  onClick={() => handleLanguageSelect(language.code)}
                  className={`w-full text-left px-3 py-2 rounded-md mb-1 transition-colors ${
                    localSelectedLanguage === language.code
                      ? 'bg-blue-50 text-blue-700 border border-blue-200'
                      : 'hover:bg-gray-50 text-gray-700'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium">{language.name}</span>
                    {localSelectedLanguage === language.code && (
                      <Check className="h-4 w-4 text-blue-600" />
                    )}
                  </div>
                  <p className="text-xs text-gray-500 mt-1">{language.code}</p>
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Footer Actions */}
        <div className="flex justify-end gap-2 p-4 border-t border-gray-200">
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
          <Button
            onClick={() => {
              onOpenChange(false);
            }}
          >
            Confirm
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

