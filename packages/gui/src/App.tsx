import { useState, useEffect } from 'react';
import { Sidebar, type ViewType } from './components/Sidebar';
import { ChatView } from './components/ChatView';
import { TaskView } from './components/TaskView';
import { SettingsPanel } from './components/SettingsPanel';
import { useSessionStore } from './stores/sessionStore';
import { useChatStore } from './stores/chatStore';
import { ServerStatusProvider } from './contexts/ServerStatusContext';
import { ToastProvider } from './contexts/ToastContext';
import { PerformanceMonitor } from './components/PerformanceMonitor';
import { LspStatusPanel } from './components/LspStatusPanel';
import { CodeGraphStatusPanel } from './components/CodeGraphStatusPanel';
import { McpStatusPanel } from './components/McpStatusPanel';
import { CommandBar } from './components/CommandBar';
import { ShortcutHelp } from './components/ShortcutHelp';
import { SessionSwitcherDialog } from './components/SessionSwitcherDialog';

function App() {
  const [currentView, setCurrentView] = useState<ViewType>('chat');
  const createSession = useSessionStore((s) => s.createSession);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const switchSession = useSessionStore((s) => s.switchSession);

  // Panel visibility states
  const [showPerformanceMonitor, setShowPerformanceMonitor] = useState(false);
  const [showLspPanel, setShowLspPanel] = useState(false);
  const [showCodeGraphPanel, setShowCodeGraphPanel] = useState(false);
  const [showMcpPanel, setShowMcpPanel] = useState(false);
  const [showCommandBar, setShowCommandBar] = useState(false);
  const [showShortcutHelp, setShowShortcutHelp] = useState(false);
  const [showSessionSwitcher, setShowSessionSwitcher] = useState(false);

  // Global keyboard shortcuts (matching TUI key handling)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Global Esc: close all dialogs (highest priority)
      if (e.key === 'Escape' && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
        if (showSessionSwitcher) {
          e.preventDefault();
          setShowSessionSwitcher(false);
          return;
        }
        if (showCommandBar) {
          e.preventDefault();
          setShowCommandBar(false);
          return;
        }
        if (showShortcutHelp) {
          e.preventDefault();
          setShowShortcutHelp(false);
          return;
        }
        if (showLspPanel) {
          e.preventDefault();
          setShowLspPanel(false);
          return;
        }
        if (showCodeGraphPanel) {
          e.preventDefault();
          setShowCodeGraphPanel(false);
          return;
        }
        if (showMcpPanel) {
          e.preventDefault();
          setShowMcpPanel(false);
          return;
        }
        if (showPerformanceMonitor) {
          e.preventDefault();
          setShowPerformanceMonitor(false);
          return;
        }
      }

      // Don't process other shortcuts when dialogs are open
      if (showCommandBar || showShortcutHelp || showSessionSwitcher) {
        return;
      }

      // Cmd/Ctrl + N: New chat
      if ((e.metaKey || e.ctrlKey) && e.key === 'n') {
        e.preventDefault();
        createSession();
        clearMessages();
        setCurrentView('chat');
      }
      // Cmd/Ctrl + T: Tasks view
      if ((e.metaKey || e.ctrlKey) && e.key === 't') {
        e.preventDefault();
        setCurrentView('tasks');
      }
      // Cmd/Ctrl + ,: Settings view
      if ((e.metaKey || e.ctrlKey) && e.key === ',') {
        e.preventDefault();
        setCurrentView('settings');
      }

      // "/" : Open command bar
      if (e.key === '/' && !e.ctrlKey && !e.altKey && !e.metaKey) {
        e.preventDefault();
        setShowCommandBar(true);
      }

      // "?" : Show shortcut help
      if (e.key === '?' && !e.ctrlKey && !e.altKey && !e.metaKey) {
        e.preventDefault();
        setShowShortcutHelp(true);
      }

      // Panel shortcuts (matching TUI)
      // Alt+L: LSP status panel
      if (e.altKey && e.key === 'l') {
        e.preventDefault();
        setShowLspPanel(prev => !prev);
      }
      // Alt+G: CodeGraph status panel
      if (e.altKey && e.key === 'g') {
        e.preventDefault();
        setShowCodeGraphPanel(prev => !prev);
      }
      // Alt+W: MCP status panel (workflow)
      if (e.altKey && e.key === 'w') {
        e.preventDefault();
        setShowMcpPanel(prev => !prev);
      }
      // Shift+D: Debug/Performance monitor
      if (e.shiftKey && e.key === 'D' && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        setShowPerformanceMonitor(prev => !prev);
      }
      // Shift+P: Performance monitor (alternative)
      if (e.shiftKey && e.key === 'P' && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        setShowPerformanceMonitor(prev => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [createSession, clearMessages, showCommandBar, showShortcutHelp, showSessionSwitcher, showLspPanel, showCodeGraphPanel, showMcpPanel, showPerformanceMonitor]);

  const renderView = () => {
    switch (currentView) {
      case 'chat':
        return <ChatView />;
      case 'tasks':
        return <TaskView />;
      case 'settings':
        return <SettingsPanel />;
      default:
        return <ChatView />;
    }
  };

  // Handle session switch
  const handleSessionSwitch = async (sessionId: string) => {
    await switchSession(sessionId);
    clearMessages();  // Clear current messages
    setCurrentView('chat');
  };

  return (
    <ToastProvider>
      <ServerStatusProvider>
        <div className="flex h-screen bg-background text-foreground">
          <Sidebar currentView={currentView} onViewChange={setCurrentView} />
          <main className="flex-1 flex flex-col min-w-0">
            {renderView()}
          </main>

          {/* Status panels (overlay) */}
          {showLspPanel && (
            <div className="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 z-40">
              <LspStatusPanel onClose={() => setShowLspPanel(false)} />
            </div>
          )}
          {showCodeGraphPanel && (
            <div className="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 z-40">
              <CodeGraphStatusPanel onClose={() => setShowCodeGraphPanel(false)} />
            </div>
          )}
          {showMcpPanel && (
            <div className="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 z-40">
              <McpStatusPanel onClose={() => setShowMcpPanel(false)} />
            </div>
          )}
          {showPerformanceMonitor && (
            <div className="fixed bottom-4 right-4 z-40">
              <PerformanceMonitor />
            </div>
          )}

          {/* Session switcher */}
          {showSessionSwitcher && (
            <SessionSwitcherDialog
              onClose={() => setShowSessionSwitcher(false)}
              onSelectSession={handleSessionSwitch}
            />
          )}

          {/* Command bar */}
          {showCommandBar && (
            <CommandBar
              onSubmitCommand={(cmd) => {
                console.log('Command submitted:', cmd);
                if (cmd === '/help' || cmd === '/shortcuts') {
                  setShowShortcutHelp(true);
                } else if (cmd === '/sessions' || cmd === '/history') {
                  setShowSessionSwitcher(true);
                }
              }}
              onClose={() => setShowCommandBar(false)}
            />
          )}

          {/* Shortcut help */}
          {showShortcutHelp && (
            <ShortcutHelp onClose={() => setShowShortcutHelp(false)} />
          )}
        </div>
      </ServerStatusProvider>
    </ToastProvider>
  );
}

export default App;