import React, { useState, useEffect } from 'react';

// Default shortcuts configuration
const DEFAULT_SHORTCUTS: Record<string, string> = {
  'send-message': 'Enter',
  'new-line': 'Shift+Enter',
  'toggle-debug': 'Ctrl+D',
  'toggle-workflow': 'Alt+W',
  'toggle-mode': 'Alt+M',
  'open-command-bar': '/',
  'show-shortcuts': 'Shift+?',
  'scroll-to-bottom': 'End',
  'clear-input': 'Ctrl+K',
  'history-up': 'ArrowUp',
  'history-down': 'ArrowDown',
};

// Action labels
const ACTION_LABELS: Record<string, string> = {
  'send-message': '发送消息',
  'new-line': '换行',
  'toggle-debug': '切换调试面板',
  'toggle-workflow': '切换工作流面板',
  'toggle-mode': '切换批准模式',
  'open-command-bar': '打开命令栏',
  'show-shortcuts': '显示快捷键帮助',
  'scroll-to-bottom': '滚动到底部',
  'clear-input': '清空输入',
  'history-up': '输入历史-上一个',
  'history-down': '输入历史-下一个',
};

// Keybinding configuration interface
interface KeybindingConfig {
  action: string;
  key: string;
  label: string;
}

// Load shortcuts from localStorage
function loadShortcuts(): Record<string, string> {
  try {
    const stored = localStorage.getItem('matrixcode-shortcuts');
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.error('Failed to load shortcuts:', e);
  }
  return DEFAULT_SHORTCUTS;
}

// Save shortcuts to localStorage
function saveShortcuts(shortcuts: Record<string, string>): void {
  try {
    localStorage.setItem('matrixcode-shortcuts', JSON.stringify(shortcuts));
  } catch (e) {
    console.error('Failed to save shortcuts:', e);
  }
}

// Hook for managing shortcuts
export function useKeybindings() {
  const [shortcuts, setShortcuts] = useState<Record<string, string>>(loadShortcuts());

  // Update shortcut
  const updateShortcut = (action: string, key: string) => {
    const newShortcuts = { ...shortcuts, [action]: key };
    setShortcuts(newShortcuts);
    saveShortcuts(newShortcuts);
  };

  // Reset to default
  const resetShortcuts = () => {
    setShortcuts(DEFAULT_SHORTCUTS);
    saveShortcuts(DEFAULT_SHORTCUTS);
  };

  // Get shortcut for action
  const getShortcut = (action: string): string => {
    return shortcuts[action] || DEFAULT_SHORTCUTS[action];
  };

  // Get all keybindings
  const getAllKeybindings = (): KeybindingConfig[] => {
    return Object.keys(ACTION_LABELS).map(action => ({
      action,
      key: shortcuts[action] || DEFAULT_SHORTCUTS[action],
      label: ACTION_LABELS[action],
    }));
  };

  return {
    shortcuts,
    updateShortcut,
    resetShortcuts,
    getShortcut,
    getAllKeybindings,
  };
}

// Keybinding editor dialog
interface KeybindingEditorDialogProps {
  onClose: () => void;
}

export function KeybindingEditorDialog({ onClose }: KeybindingEditorDialogProps) {
  const { shortcuts, updateShortcut, resetShortcuts, getAllKeybindings } = useKeybindings();
  const [editingAction, setEditingAction] = useState<string | null>(null);
  const [pressedKey, setPressedKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const keybindings = getAllKeybindings();

  // Handle key capture
  useEffect(() => {
    if (editingAction) {
      const handleKeyDown = (e: KeyboardEvent) => {
        e.preventDefault();
        e.stopPropagation();

        // Build key combination
        const parts: string[] = [];
        if (e.ctrlKey) parts.push('Ctrl');
        if (e.metaKey) parts.push('Cmd');
        if (e.altKey) parts.push('Alt');
        if (e.shiftKey) parts.push('Shift');

        // Key name
        let key = e.key;
        if (key === ' ') key = 'Space';
        if (key === 'ArrowUp') key = '↑';
        if (key === 'ArrowDown') key = '↓';
        if (key === 'ArrowLeft') key = '←';
        if (key === 'ArrowRight') key = '→';
        if (key.startsWith('Arrow')) key = e.key;
        if (key.length === 1) key = key.toUpperCase();

        parts.push(key);
        const combination = parts.join('+');

        setPressedKey(combination);

        // Check for conflicts
        const conflicts = keybindings.filter(kb =>
          kb.key === combination && kb.action !== editingAction
        );
        if (conflicts.length > 0) {
          setError(`快捷键冲突: ${conflicts.map(c => c.label).join(', ')}`);
        } else {
          setError(null);
        }
      };

      const handleKeyUp = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          // Cancel editing
          setEditingAction(null);
          setPressedKey(null);
          setError(null);
          return;
        }

        if (e.key === 'Enter' && pressedKey) {
          // Confirm shortcut
          if (!error) {
            updateShortcut(editingAction, pressedKey);
          }
          setEditingAction(null);
          setPressedKey(null);
          setError(null);
        }
      };

      window.addEventListener('keydown', handleKeyDown);
      window.addEventListener('keyup', handleKeyUp);

      return () => {
        window.removeEventListener('keydown', handleKeyDown);
        window.removeEventListener('keyup', handleKeyUp);
      };
    }
  }, [editingAction, pressedKey, error, keybindings, updateShortcut]);

  // Format key for display
  const formatKey = (key: string): string => {
    return key.replace('Ctrl', '⌃').replace('Cmd', '⌘').replace('Alt', '⌥').replace('Shift', '⇧');
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2">
              <span>⌨️</span>
              <span>Customize Shortcuts</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
          <p className="text-sm text-muted-foreground mt-1">
            自定义键盘快捷键，按 Enter 确认，Esc 取消
          </p>
        </div>

        {/* Editing indicator */}
        {editingAction && (
          <div className="px-4 py-3 bg-primary/10 border-b">
            <div className="text-sm">
              正在编辑: <span className="font-medium">{ACTION_LABELS[editingAction]}</span>
            </div>
            <div className="flex items-center gap-2 mt-2">
              <span className="text-xs text-muted-foreground">按下快捷键:</span>
              {pressedKey && (
                <kbd className="px-3 py-1.5 bg-primary text-primary-foreground rounded text-sm font-mono">
                  {formatKey(pressedKey)}
                </kbd>
              )}
              {!pressedKey && (
                <span className="text-xs text-muted-foreground animate-pulse">
                  等待输入...
                </span>
              )}
            </div>
            {error && (
              <div className="text-xs text-red-500 mt-1">
                {error}
              </div>
            )}
          </div>
        )}

        {/* Shortcuts list */}
        <div className="max-h-[400px] overflow-y-auto">
          {keybindings.map((kb) => (
            <div
              key={kb.action}
              onClick={() => {
                if (!editingAction) {
                  setEditingAction(kb.action);
                }
              }}
              className={`px-4 py-3 flex items-center justify-between cursor-pointer border-b transition-colors ${
                editingAction === kb.action ? 'bg-primary/10' : 'hover:bg-accent/30'
              }`}
            >
              {/* Action label */}
              <span className="text-sm">{kb.label}</span>

              {/* Key */}
              <div className="flex items-center gap-2">
                <kbd className="px-3 py-1.5 bg-muted rounded text-sm font-mono border">
                  {formatKey(kb.key)}
                </kbd>
                {/* Edit indicator */}
                {editingAction === kb.action && (
                  <span className="text-xs text-primary animate-pulse">
                    编辑中
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex gap-2">
            <button
              onClick={resetShortcuts}
              className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
            >
              重置为默认
            </button>
            <button
              onClick={onClose}
              className="flex-1 px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm hover:bg-primary/90 transition-colors"
            >
              完成
            </button>
          </div>
          <div className="text-xs text-muted-foreground mt-2">
            点击快捷键进行编辑
          </div>
        </div>
      </div>
    </div>
  );
}

// Get human-readable shortcut description
export function getShortcutDescription(action: string): string {
  const shortcuts = loadShortcuts();
  const key = shortcuts[action] || DEFAULT_SHORTCUTS[action];
  return `${ACTION_LABELS[action]} (${key})`;
}