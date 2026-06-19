import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface SessionInfo {
  id: string;
  name: string;
  message_count: number;
  created_at: string;
}

interface SessionSwitcherDialogProps {
  onClose: () => void;
  onSelectSession: (sessionId: string) => void;
}

export function SessionSwitcherDialog({ onClose, onSelectSession }: SessionSwitcherDialogProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [filter, setFilter] = useState('');
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Load sessions from backend
  useEffect(() => {
    loadSessions();
  }, []);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const loadSessions = async () => {
    try {
      setLoading(true);
      setError(null);
      const sessionList = await invoke<SessionInfo[]>('list_sessions');
      setSessions(sessionList || []);
      setSelectedIndex(0);
    } catch (e) {
      console.error('Failed to load sessions:', e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  // Filtered sessions
  const filteredSessions = sessions.filter(s =>
    s.name.toLowerCase().includes(filter.toLowerCase()) ||
    s.id.toLowerCase().includes(filter.toLowerCase())
  );

  // Keyboard navigation (matching TUI session selection)
  useEffect(() => {
    const handleKeyDown = (e: globalThis.KeyboardEvent) => {
      if (e.key === 'ArrowDown' || e.key === 'j' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setSelectedIndex(i => Math.min(i + 1, filteredSessions.length - 1));
      } else if (e.key === 'ArrowUp' || e.key === 'k' && !e.ctrlKey && !e.metaKey) {
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
          <h2 className="text-lg font-semibold">Session Switcher</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-accent rounded text-muted-foreground hover:text-foreground transition-colors"
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
            placeholder="Search sessions... (↑/↓ or j/k to navigate, Enter to select)"
            className="w-full px-3 py-2 bg-background border border-input rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
          />
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {loading ? (
            <div className="text-center py-8 text-muted-foreground">
              Loading sessions...
            </div>
          ) : error ? (
            <div className="text-center py-8 text-red-500">
              <p className="mb-2">Failed to load sessions</p>
              <p className="text-sm">{error}</p>
              <button
                onClick={loadSessions}
                className="mt-4 px-3 py-1.5 bg-primary text-primary-foreground rounded-lg hover:bg-primary/90"
              >
                Retry
              </button>
            </div>
          ) : filteredSessions.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              {filter ? 'No sessions match filter' : 'No saved sessions'}
            </div>
          ) : (
            <div className="space-y-2">
              {filteredSessions.map((session, idx) => (
                <div
                  key={session.id}
                  onClick={() => handleSessionClick(session.id, idx)}
                  className={`border rounded-lg p-3 cursor-pointer transition-colors ${
                    idx === selectedIndex
                      ? 'bg-primary/10 border-primary'
                      : 'bg-background hover:bg-accent'
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <div className="font-medium">
                      {session.name || 'Unnamed Session'}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {session.message_count} messages
                    </div>
                  </div>
                  <div className="text-sm text-muted-foreground flex items-center gap-2">
                    <span className="text-xs font-mono">{session.id.slice(0, 8)}</span>
                    <span>•</span>
                    <span>{session.created_at}</span>
                  </div>
                  {idx === selectedIndex && (
                    <div className="mt-2 text-xs text-primary">
                      Press Enter to select
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t p-4 flex justify-between items-center">
          <div className="text-xs text-muted-foreground">
            {filteredSessions.length} session(s)
          </div>
          <div className="text-xs text-muted-foreground flex items-center gap-2">
            <span>
              <kbd className="px-1 bg-accent rounded">↑↓</kbd> or
              <kbd className="px-1 bg-accent rounded">j/k</kbd> navigate
            </span>
            <span>
              <kbd className="px-1 bg-accent rounded">Enter</kbd> select
            </span>
            <span>
              <kbd className="px-1 bg-accent rounded">Esc</kbd> cancel
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}