import React, { useEffect, useState } from 'react';
import { useSessionStore, type SessionInfo } from '../stores/sessionStore';
import { useChatStore } from '../stores/chatStore';

export type ViewType = 'chat' | 'tasks' | 'settings';

interface SidebarProps {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
}

export function Sidebar({ currentView, onViewChange }: SidebarProps) {
  const sessions = useSessionStore((s) => s.sessions);
  const currentSessionId = useSessionStore((s) => s.currentSessionId);
  const loadSessions = useSessionStore((s) => s.loadSessions);
  const createSession = useSessionStore((s) => s.createSession);
  const continueLast = useSessionStore((s) => s.continueLast);
  const switchSession = useSessionStore((s) => s.switchSession);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const loadMessages = useChatStore((s) => s.loadMessages);
  const [collapsed, setCollapsed] = useState(false);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleNew = async () => {
    await createSession();
    clearMessages();
    onViewChange('chat');
  };

  const handleContinue = async () => {
    const id = await continueLast();
    if (id) {
      // Load messages from the resumed session
      await loadMessages();
    }
    onViewChange('chat');
  };

  const handleSelect = async (session: SessionInfo) => {
    // Actually switch the backend session and load its messages
    await switchSession(session.id);
    await loadMessages();
    onViewChange('chat');
  };

  if (collapsed) {
    return (
      <div className="w-12 border-r flex flex-col items-center py-3 gap-3 bg-card">
        <button
          onClick={() => setCollapsed(false)}
          className="p-1.5 hover:bg-accent rounded text-sm"
          title="Expand sidebar"
        >
          ▶
        </button>
        <button
          onClick={handleNew}
          className="p-1.5 hover:bg-accent rounded text-sm"
          title="New Chat"
        >
          +
        </button>
        <button
          onClick={() => onViewChange('tasks')}
          className={`p-1.5 rounded text-sm ${currentView === 'tasks' ? 'bg-accent' : 'hover:bg-accent'}`}
          title="Tasks"
        >
          ☐
        </button>
        <button
          onClick={() => onViewChange('settings')}
          className={`p-1.5 rounded text-sm ${currentView === 'settings' ? 'bg-accent' : 'hover:bg-accent'}`}
          title="Settings"
        >
          ⚙
        </button>
      </div>
    );
  }

  return (
    <div className="w-64 border-r flex flex-col h-full bg-card">
      {/* Header */}
      <div className="p-3 border-b">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-bold">MatrixCode</h1>
          <button
            onClick={() => setCollapsed(true)}
            className="p-1 hover:bg-accent rounded text-xs text-muted-foreground"
            title="Collapse sidebar"
          >
            ◀
          </button>
        </div>
        <div className="flex gap-2 mt-2">
          <button
            onClick={handleNew}
            className="flex-1 text-xs px-2 py-1.5 bg-primary text-primary-foreground rounded hover:bg-primary/90 transition-colors"
          >
            New Chat
          </button>
          <button
            onClick={handleContinue}
            className="text-xs px-2 py-1.5 border rounded hover:bg-accent transition-colors"
          >
            Continue
          </button>
        </div>
      </div>

      {/* Navigation tabs */}
      <div className="flex border-b">
        {[
          { key: 'chat' as ViewType, label: '💬 Chat' },
          { key: 'tasks' as ViewType, label: '☐ Tasks' },
          { key: 'settings' as ViewType, label: '⚙ Settings' },
        ].map(({ key, label }) => (
          <button
            key={key}
            onClick={() => onViewChange(key)}
            className={`flex-1 text-xs py-2 transition-colors ${
              currentView === key
                ? 'border-b-2 border-primary font-medium'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Session list (only shown in chat view) */}
      {currentView === 'chat' && (
        <div className="flex-1 overflow-y-auto">
          {sessions.length === 0 && (
            <p className="text-xs text-muted-foreground p-3">No sessions yet</p>
          )}
          {sessions.map((s) => (
            <button
              key={s.id}
              onClick={() => handleSelect(s)}
              className={`w-full text-left px-3 py-2 text-sm border-b hover:bg-accent transition-colors ${
                s.id === currentSessionId
                  ? 'bg-accent font-medium'
                  : ''
              }`}
            >
              <div className="truncate">{s.name || `Session ${s.id.slice(0, 8)}`}</div>
              <div className="text-xs text-muted-foreground">
                {s.message_count} msgs · {s.created_at}
              </div>
            </button>
          ))}
        </div>
      )}

      {currentView !== 'chat' && (
        <div className="flex-1" />
      )}

      {/* Footer */}
      <div className="p-3 border-t text-xs text-muted-foreground">
        v0.1.0
      </div>
    </div>
  );
}