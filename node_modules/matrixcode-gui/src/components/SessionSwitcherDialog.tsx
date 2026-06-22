import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface SessionInfo {
  id: string;
  name: string;
  message_count: number;
  created_at: string;
  project_path?: string;
  updated_at?: string;
  short_id?: string;
}

interface SessionSwitcherDialogProps {
  onClose: () => void;
  onSelectSession: (sessionId: string) => void;
  currentSessionId?: string | null;
}

export function SessionSwitcherDialog({ onClose, onSelectSession, currentSessionId }: SessionSwitcherDialogProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [filter, setFilter] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const loadSessions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const sessionList = await invoke<SessionInfo[]>('list_sessions');
      setSessions(sessionList || []);
      // Find the current session index
      if (currentSessionId) {
        const idx = sessionList.findIndex(s => s.id === currentSessionId);
        if (idx >= 0) {
          setSelectedIndex(idx);
        } else {
          setSelectedIndex(0);
        }
      }
    } catch (e) {
      console.error('Failed to load sessions:', e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [currentSessionId]);

  // Load sessions from backend
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleDeleteSession = async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();

    // Confirm deletion
    if (!window.confirm('确定要删除此会话吗？此操作不可恢复。')) {
      return;
    }

    try {
      setDeleting(sessionId);
      await invoke('delete_session', { id: sessionId });
      await loadSessions();
    } catch (err) {
      console.error('Failed to delete session:', err);
      setError(`删除失败: ${err}`);
    } finally {
      setDeleting(null);
    }
  };

  // Filtered sessions
  const filteredSessions = sessions.filter(s =>
    s.name.toLowerCase().includes(filter.toLowerCase()) ||
    s.id.toLowerCase().includes(filter.toLowerCase()) ||
    (s.project_path && s.project_path.toLowerCase().includes(filter.toLowerCase())) ||
    (s.short_id && s.short_id.toLowerCase().includes(filter.toLowerCase()))
  );

  // Keyboard navigation (matching TUI session selection)
  useEffect(() => {
    const handleKeyDown = (e: globalThis.KeyboardEvent) => {
      if (e.key === 'ArrowDown' || (e.key === 'j' && !e.ctrlKey && !e.metaKey)) {
        e.preventDefault();
        setSelectedIndex(i => Math.min(i + 1, filteredSessions.length - 1));
      } else if (e.key === 'ArrowUp' || (e.key === 'k' && !e.ctrlKey && !e.metaKey)) {
        e.preventDefault();
        setSelectedIndex(i => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (filteredSessions.length > 0) {
          onSelectSession(filteredSessions[selectedIndex].id);
          onClose();
        }
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [filteredSessions, selectedIndex, onClose, onSelectSession]);

  // Handle session click
  const handleSessionClick = (sessionId: string, index: number) => {
    setSelectedIndex(index);
    onSelectSession(sessionId);
    onClose();
  };

  // Extract project name from path
  const getProjectName = (path?: string) => {
    if (!path) return null;
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || path;
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" onClick={(e) => {
      // Close on background click
      if (e.target === e.currentTarget) {
        onClose();
      }
    }}>
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b">
          <h2 className="text-lg font-semibold">切换会话</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-accent rounded text-muted-foreground hover:text-foreground transition-colors"
            aria-label="关闭"
          >
            ✕
          </button>
        </div>

        {/* Search filter */}
        <div className="p-4 border-b">
          <input
            ref={inputRef}
            type="text"
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value);
              setSelectedIndex(0);  // Reset selection when filter changes
            }}
            placeholder="搜索会话名称、ID 或项目路径..."
            className="w-full px-3 py-2 bg-background border border-input rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
            aria-label="搜索会话"
          />
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {loading ? (
            <div className="text-center py-8 text-muted-foreground">
              正在加载会话...
            </div>
          ) : error ? (
            <div className="text-center py-8 text-red-500">
              <p className="mb-2">加载会话失败</p>
              <p className="text-sm">{error}</p>
              <button
                onClick={loadSessions}
                className="mt-4 px-3 py-1.5 bg-primary text-primary-foreground rounded-lg hover:bg-primary/90"
              >
                重试
              </button>
            </div>
          ) : filteredSessions.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              {filter ? '没有匹配的会话' : '没有保存的会话'}
            </div>
          ) : (
            <div className="space-y-2">
              {filteredSessions.map((session, idx) => {
                const isCurrent = session.id === currentSessionId;
                const isDeleting = deleting === session.id;

                return (
                  <div
                    key={session.id}
                    onClick={() => !isDeleting && handleSessionClick(session.id, idx)}
                    className={`border rounded-lg p-3 cursor-pointer transition-colors ${
                      isDeleting
                        ? 'opacity-50 cursor-not-allowed'
                        : idx === selectedIndex
                          ? 'bg-primary/10 border-primary'
                          : 'bg-background hover:bg-accent'
                    } ${isCurrent ? 'ring-2 ring-primary/50' : ''}`}
                    aria-label={`会话: ${session.name || '未命名会话'}`}
                    aria-current={isCurrent ? 'true' : 'false'}
                  >
                    <div className="flex items-center justify-between mb-1">
                      <div className="flex items-center gap-2">
                        {/* Current session indicator */}
                        {isCurrent && (
                          <span className="text-primary font-bold" aria-label="当前会话">
                            *
                          </span>
                        )}
                        <div className="font-medium">
                          {session.name || '未命名会话'}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <div className="text-xs text-muted-foreground">
                          {session.message_count} 条消息
                        </div>
                        {/* Delete button */}
                        <button
                          onClick={(e) => handleDeleteSession(session.id, e)}
                          disabled={isDeleting || isCurrent}
                          className={`p-1 rounded transition-colors ${
                            isCurrent
                              ? 'text-muted-foreground/50 cursor-not-allowed'
                              : 'text-muted-foreground hover:text-red-500 hover:bg-red-500/10'
                          }`}
                          aria-label={`删除会话 ${session.name || session.short_id}`}
                          title={isCurrent ? '无法删除当前会话' : '删除会话'}
                        >
                          {isDeleting ? '...' : '🗑'}
                        </button>
                      </div>
                    </div>

                    {/* Metadata display */}
                    <div className="text-sm text-muted-foreground flex flex-wrap items-center gap-2">
                      <span className="text-xs font-mono bg-accent px-1 rounded">
                        {session.short_id || session.id.slice(0, 8)}
                      </span>
                      <span>•</span>
                      <span>创建: {session.created_at}</span>
                      {session.updated_at && (
                        <>
                          <span>•</span>
                          <span>更新: {session.updated_at}</span>
                        </>
                      )}
                      {session.project_path && (
                        <>
                          <span>•</span>
                          <span className="truncate max-w-[150px]" title={session.project_path}>
                            项目: {getProjectName(session.project_path)}
                          </span>
                        </>
                      )}
                    </div>

                    {/* Keyboard hint for selected item */}
                    {idx === selectedIndex && !isDeleting && (
                      <div className="mt-2 text-xs text-primary">
                        按 Enter 选择此会话
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t p-4 flex justify-between items-center">
          <div className="text-xs text-muted-foreground">
            {filteredSessions.length} 个会话
            {currentSessionId && filteredSessions.some(s => s.id === currentSessionId) && (
              <span className="ml-2 text-primary">(* 当前会话)</span>
            )}
          </div>
          <div className="text-xs text-muted-foreground flex items-center gap-2">
            <span>
              <kbd className="px-1 bg-accent rounded">↑↓</kbd>
              <span className="ml-1">或</span>
              <kbd className="px-1 bg-accent rounded ml-1">j/k</kbd>
              <span className="ml-1">导航</span>
            </span>
            <span>
              <kbd className="px-1 bg-accent rounded">Enter</kbd>
              <span className="ml-1">选择</span>
            </span>
            <span>
              <kbd className="px-1 bg-accent rounded">Esc</kbd>
              <span className="ml-1">取消</span>
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}