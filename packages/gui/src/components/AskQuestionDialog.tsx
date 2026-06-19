import React, { useState } from 'react';

interface AskQuestionProps {
  question: string;
  options?: Record<string, string> | string[];
  onAnswer: (answer: string) => void;
  onCancel?: () => void;
}

export function AskQuestionDialog({ question, options, onAnswer, onCancel }: AskQuestionProps) {
  const [selectedOption, setSelectedOption] = useState<string>('');
  const [customAnswer, setCustomAnswer] = useState<string>('');

  // Parse options - could be array or object
  const parseOptions = (): Array<{ key: string; label: string }> => {
    if (!options) return [];

    if (Array.isArray(options)) {
      return options.map((opt, idx) => ({
        key: String(idx),
        label: String(opt),
      }));
    }

    if (typeof options === 'object') {
      return Object.entries(options).map(([key, value]) => ({
        key,
        label: String(value),
      }));
    }

    return [];
  };

  const parsedOptions = parseOptions();
  const hasOptions = parsedOptions.length > 0;

  const handleSubmit = () => {
    if (hasOptions && selectedOption) {
      // If options provided, send the selected option key
      onAnswer(selectedOption);
    } else if (customAnswer.trim()) {
      // Otherwise send custom text answer
      onAnswer(customAnswer.trim());
    }
  };

  const canSubmit = hasOptions ? selectedOption !== '' : customAnswer.trim() !== '';

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full p-6 space-y-4">
        {/* Header */}
        <div className="flex items-center gap-2 pb-3 border-b">
          <span className="text-2xl">❓</span>
          <h3 className="text-lg font-semibold">Agent Question</h3>
        </div>

        {/* Question */}
        <div className="bg-muted/50 rounded-lg p-4">
          <p className="text-sm whitespace-pre-wrap">{question}</p>
        </div>

        {/* Options or Text Input */}
        {hasOptions ? (
          <div className="space-y-2">
            <label className="text-sm font-medium text-muted-foreground">
              Choose an option:
            </label>
            <div className="space-y-1.5">
              {parsedOptions.map(({ key, label }) => (
                <button
                  key={key}
                  onClick={() => setSelectedOption(key)}
                  className={`w-full text-left px-4 py-2.5 rounded-lg border transition-colors ${
                    selectedOption === key
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-background hover:bg-muted border-border'
                  }`}
                >
                  <span className="font-mono text-xs opacity-70 mr-2">[{key}]</span>
                  <span className="text-sm">{label}</span>
                </button>
              ))}
            </div>
          </div>
        ) : (
          <div className="space-y-2">
            <label className="text-sm font-medium text-muted-foreground">
              Your answer:
            </label>
            <textarea
              value={customAnswer}
              onChange={(e) => setCustomAnswer(e.target.value)}
              placeholder="Type your answer here..."
              className="w-full rounded-lg border bg-background px-3 py-2.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring min-h-[80px] resize-none"
              autoFocus
            />
          </div>
        )}

        {/* Actions */}
        <div className="flex gap-2 pt-3 border-t">
          {onCancel && (
            <button
              onClick={onCancel}
              className="px-4 py-2 text-sm rounded-lg border bg-background hover:bg-muted transition-colors"
            >
              Cancel
            </button>
          )}
          <button
            onClick={handleSubmit}
            disabled={!canSubmit}
            className="flex-1 px-4 py-2 text-sm font-medium rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            Submit Answer
          </button>
        </div>
      </div>
    </div>
  );
}