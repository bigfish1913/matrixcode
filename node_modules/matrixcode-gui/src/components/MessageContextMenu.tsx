import React, { useState, useRef, useEffect } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useToastContext } from '../contexts/ToastContext';

// Context menu action (matching VSCode extension editor/context menu)
interface ContextMenuAction {
  id: string;
  label: string;
  icon: string;
  action: (selectedText: string) => void;
  requiresSelection: boolean;
}

interface ContextMenuProps {
  selectedText: string;
  position: { x: number; y: number };
  onClose: () => void;
}

export function MessageContextMenu({ selectedText, position, onClose }: ContextMenuProps) {
  const sendMessage = useChatStore((s) => s.sendMessage);
  const status = useChatStore((s) => s.status);
  const toast = useToastContext();
  const menuRef = useRef<HTMLDivElement>(null);

  // Context menu actions (matching VSCode extension matrixcode.submenu)
  const actions: ContextMenuAction[] = [
    {
      id: 'explain',
      label: '解释选中代码',
      icon: '💡',
      action: (text) => sendMessage(`请解释这段代码:\n\n${text}`),
      requiresSelection: true,
    },
    {
      id: 'fix',
      label: '修复代码问题',
      icon: '🔧',
      action: (text) => sendMessage(`请修复这段代码的问题:\n\n${text}`),
      requiresSelection: true,
    },
    {
      id: 'refactor',
      label: '重构代码',
      icon: '🔄',
      action: (text) => sendMessage(`请重构这段代码:\n\n${text}`),
      requiresSelection: true,
    },
    {
      id: 'tests',
      label: '生成单元测试',
      icon: '🧪',
      action: (text) => sendMessage(`请为这段代码生成单元测试:\n\n${text}`),
      requiresSelection: true,
    },
    {
      id: 'improve',
      label: '改进代码质量',
      icon: '⬆️',
      action: (text) => sendMessage(`请改进这段代码的质量:\n\n${text}`),
      requiresSelection: true,
    },
    {
      id: 'ask',
      label: '自定义问题...',
      icon: '❓',
      action: (text) => {
        // This should open a custom question dialog with the selected text
        toast.addToast({ type: 'info', message: '请在输入框中输入您的问题，选中的代码会自动附带' });
        // Pre-fill QuickActionPanel with the selected text
        console.log('Selected text for custom question:', text);
      },
      requiresSelection: true,
    },
    {
      id: 'copy',
      label: '复制',
      icon: '📋',
      action: async (text) => {
        try {
          await navigator.clipboard.writeText(text);
          toast.addToast({ type: 'success', message: '✓ 已复制到剪贴板' });
        } catch (e) {
          toast.addToast({ type: 'error', message: '复制失败' });
        }
      },
      requiresSelection: true,
    },
  ];

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  // Close menu on Escape
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [onClose]);

  const handleActionClick = (action: ContextMenuAction) => {
    if (status === 'running') {
      toast.addToast({ type: 'warning', message: '请等待当前任务完成' });
      return;
    }

    if (action.requiresSelection && !selectedText.trim()) {
      toast.addToast({ type: 'info', message: '请先选择文本' });
      return;
    }

    action.action(selectedText);
    onClose();
  };

  const filteredActions = selectedText.trim()
    ? actions
    : actions.filter(a => !a.requiresSelection);

  return (
    <div
      ref={menuRef}
      className="fixed bg-card border shadow-lg rounded-lg py-1 z-50 animate-fade-in min-w-[180px]"
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
      }}
    >
      {/* Header */}
      <div className="px-3 py-1.5 text-xs text-muted-foreground border-b">
        MatrixCode 快捷操作
      </div>

      {/* Menu items */}
      {filteredActions.map((action, idx) => (
        <button
          key={action.id}
          onClick={() => handleActionClick(action)}
          disabled={status === 'running'}
          className="w-full px-3 py-2 text-sm hover:bg-accent transition-colors flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <span className="text-base">{action.icon}</span>
          <span>{action.label}</span>
        </button>
      ))}

      {/* Hint */}
      {selectedText.trim() && (
        <div className="px-3 py-1.5 text-xs text-muted-foreground border-t">
          已选中: {selectedText.length > 30 ? selectedText.slice(0, 30) + '...' : selectedText} ({selectedText.length} 字符)
        </div>
      )}
    </div>
  );
}