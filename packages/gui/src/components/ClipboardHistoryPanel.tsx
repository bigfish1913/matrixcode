import React, { useState, useEffect } from 'react';
import { useChatStore } from '../stores/chatStore';

interface ClipboardHistoryPanelProps {
  onClose: () => void;
}

interface ClipboardItem {
  id: string;
  content: string;
  source: 'message' | 'code' | 'tool_result' | 'manual';
  timestamp: number;
  preview?: string;
}

// Clipboard history store (in-memory for session)
const clipboardHistory: ClipboardItem[] = [];
const MAX_HISTORY = 20;

// Add to clipboard history
export function addToClipboardHistory(
  content: string,
  source: ClipboardItem['source'] = 'manual'
): void {
  const item: ClipboardItem = {
    id: `clip-${Date.now()}`,
    content,
    source,
    timestamp: Date.now(),
    preview: content.slice(0, 100) + (content.length > 100 ? '...' : ''),
  };

  // Deduplicate
  const exists = clipboardHistory.find(i => i.content === content);
  if (exists) {
    exists.timestamp = Date.now();
    return;
  }

  clipboardHistory.unshift(item);
  if (clipboardHistory.length > MAX_HISTORY) {
    clipboardHistory.pop();
  }
}

// Get clipboard history
export function getClipboardHistory(): ClipboardItem[] {
  return [...clipboardHistory];
}

// Clear clipboard history
export function clearClipboardHistory(): void {
  clipboardHistory.length = 0;
}

export function ClipboardHistoryPanel({ onClose }: ClipboardHistoryPanelProps) {
  const [history, setHistory] = useState<ClipboardItem[]>([]);
  const [selectedItems, setSelectedItems] = useState<Set<string>>(new Set());

  // Load history on mount
  useEffect(() => {
    setHistory(getClipboardHistory());
  }, []);

  // Toggle item selection
  const toggleSelection = (id: string) => {
    const newSelection = new Set(selectedItems);
    if (newSelection.has(id)) {
      newSelection.delete(id);
    } else {
      newSelection.add(id);
    }
    setSelectedItems(newSelection);
  };

  // Copy selected items
  const copySelected = async () => {
    const selectedContent = history
      .filter(item => selectedItems.has(item.id))
      .map(item => item.content)
      .join('\n\n---\n\n');

    await navigator.clipboard.writeText(selectedContent);
    onClose();
  };

  // Copy single item
  const copySingle = async (content: string) => {
    await navigator.clipboard.writeText(content);
    addToClipboardHistory(content, 'manual');
    onClose();
  };

  // Delete item
  const deleteItem = (id: string) => {
    const idx = clipboardHistory.findIndex(i => i.id === id);
    if (idx >= 0) {
      clipboardHistory.splice(idx, 1);
      setHistory(getClipboardHistory());
    }
    setSelectedItems(new Set()); // Reset selection
  };

  // Clear all
  const clearAll = () => {
    clearClipboardHistory();
    setHistory([]);
    setSelectedItems(new Set());
  };

  // Source icons
  const SOURCE_ICONS: Record<string, string> = {
    message: '💬',
    code: '💻',
    tool_result: '🔧',
    manual: '📋',
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-lg w-full max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2">
              <span>📋</span>
              <span>Clipboard History</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
          <p className="text-sm text-muted-foreground mt-1">
            复制历史记录，支持批量复制
          </p>
        </div>

        {/* Selection actions */}
        {selectedItems.size > 0 && (
          <div className="px-4 py-2 bg-primary/10 border-b flex items-center gap-3">
            <span className="text-sm text-primary">
              已选择 {selectedItems.size} 项
            </span>
            <button
              onClick={copySelected}
              className="px-3 py-1 bg-primary text-primary-foreground rounded text-sm hover:bg-primary/90 transition-colors"
            >
              复制选中
            </button>
            <button
              onClick={() => setSelectedItems(new Set())}
              className="px-3 py-1 bg-muted text-muted-foreground rounded text-sm hover:bg-accent transition-colors"
            >
              取消选择
            </button>
          </div>
        )}

        {/* History list */}
        <div className="flex-1 overflow-y-auto">
          {history.length === 0 ? (
            <div className="text-center text-muted-foreground py-8">
              <div className="text-4xl mb-2">📋</div>
              <span className="text-sm">剪贴板历史为空</span>
              <p className="text-xs mt-1">
                复制代码或消息后会自动记录
              </p>
            </div>
          ) : (
            history.map((item) => (
              <div
                key={item.id}
                className={`px-4 py-3 border-b cursor-pointer transition-colors ${
                  selectedItems.has(item.id) ? 'bg-primary/10' : 'hover:bg-accent/30'
                }`}
                onClick={() => toggleSelection(item.id)}
              >
                <div className="flex items-start gap-3">
                  {/* Checkbox */}
                  <input
                    type="checkbox"
                    checked={selectedItems.has(item.id)}
                    onChange={() => toggleSelection(item.id)}
                    className="mt-1"
                  />

                  {/* Source icon */}
                  <span className="text-lg">
                    {SOURCE_ICONS[item.source]}
                  </span>

                  {/* Content preview */}
                  <div className="flex-1">
                    <pre className="text-sm text-muted-foreground font-mono whitespace-pre-wrap truncate">
                      {item.preview || item.content}
                    </pre>

                    {/* Metadata */}
                    <div className="flex items-center gap-2 mt-2 text-xs text-muted-foreground">
                      <span>{item.content.length} chars</span>
                      <span>•</span>
                      <span>{new Date(item.timestamp).toLocaleTimeString()}</span>
                    </div>
                  </div>

                  {/* Action buttons */}
                  <div className="flex gap-1">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        copySingle(item.content);
                      }}
                      className="px-2 py-1 text-xs bg-muted/50 text-muted-foreground hover:bg-muted rounded transition-colors"
                      title="Copy this item"
                    >
                      📋
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        deleteItem(item.id);
                      }}
                      className="px-2 py-1 text-xs bg-muted/50 text-muted-foreground hover:bg-red-500/20 hover:text-red-500 rounded transition-colors"
                      title="Delete this item"
                    >
                      ✕
                    </button>
                  </div>
                </div>
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
            >
              关闭
            </button>
            {history.length > 0 && (
              <button
                onClick={clearAll}
                className="px-4 py-2 bg-red-500/10 text-red-500 rounded-lg text-sm hover:bg-red-500/20 transition-colors"
              >
                清空历史
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}