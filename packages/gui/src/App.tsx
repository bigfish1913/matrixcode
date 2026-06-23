import { useState, useEffect, lazy, Suspense } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Sidebar, type ViewType } from './components/Sidebar';
import { ChatView } from './components/ChatView';
import { useSessionStore } from './stores/sessionStore';
import { useChatStore } from './stores/chatStore';
import { ServerStatusProvider } from './contexts/ServerStatusContext';
import { ToastProvider, useToastContext } from './contexts/ToastContext';
import { PerformanceMonitor } from './components/PerformanceMonitor';
import { CommandBar } from './components/CommandBar';
import { ShortcutHelp } from './components/ShortcutHelp';
import { LoopTaskIndicator } from './components/LoopTaskIndicator';
import { LoadingFallback } from './components/LoadingComponents';
import {
  ErrorBoundary,
  DialogErrorBoundary,
  DialogSkeleton,
  ShortcutHintIndicator
} from './components/shared';

// Lazy load dialogs and panels (Phase 5 optimization)
const TaskViewLazy = lazy(() =>
  import('./components/TaskView').then(module => ({ default: module.TaskView }))
);
const SettingsPanel = lazy(() =>
  import('./components/SettingsPanel').then(module => ({ default: module.SettingsPanel }))
);
const LspStatusPanel = lazy(() =>
  import('./components/LspStatusPanel').then(module => ({ default: module.LspStatusPanel }))
);
const CodeGraphStatusPanel = lazy(() =>
  import('./components/CodeGraphStatusPanel').then(module => ({ default: module.CodeGraphStatusPanel }))
);
const McpStatusPanel = lazy(() =>
  import('./components/McpStatusPanel').then(module => ({ default: module.McpStatusPanel }))
);
const SessionSwitcherDialog = lazy(() =>
  import('./components/SessionSwitcherDialog').then(module => ({ default: module.SessionSwitcherDialog }))
);
const ApproveModeDialog = lazy(() =>
  import('./components/ApproveModeDialog').then(module => ({ default: module.ApproveModeDialog }))
);
const ModelSwitcherDialog = lazy(() =>
  import('./components/ModelSwitcherDialog').then(module => ({ default: module.ModelSwitcherDialog }))
);
const LoopTaskDialog = lazy(() =>
  import('./components/LoopTaskDialog').then(module => ({ default: module.LoopTaskDialog }))
);
const CronTaskDialog = lazy(() =>
  import('./components/CronTaskDialog').then(module => ({ default: module.CronTaskDialog }))
);
const MemoryPanel = lazy(() =>
  import('./components/MemoryPanel').then(module => ({ default: module.MemoryPanel }))
);
const ToolsSkillsPanel = lazy(() =>
  import('./components/ToolsSkillsPanel').then(module => ({ default: module.ToolsSkillsPanel }))
);

// Enhanced loading fallback component is imported from LoadingComponents.tsx

// Inner component that uses toast context (must be inside ToastProvider)
function AppContent() {
  const [currentView, setCurrentView] = useState<ViewType>('chat');
  const createSession = useSessionStore((s) => s.createSession);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const loadMessages = useChatStore((s) => s.loadMessages);
  const switchSession = useSessionStore((s) => s.switchSession);
  const currentSessionId = useSessionStore((s) => s.currentSessionId);

  // Get toast at component level for panic handler (must be inside ToastProvider)
  const toast = useToastContext();

  // Panel visibility states
  const [showPerformanceMonitor, setShowPerformanceMonitor] = useState(false);
  const [showLspPanel, setShowLspPanel] = useState(false);
  const [showCodeGraphPanel, setShowCodeGraphPanel] = useState(false);
  const [showMcpPanel, setShowMcpPanel] = useState(false);
  const [showCommandBar, setShowCommandBar] = useState(false);
  const [showShortcutHelp, setShowShortcutHelp] = useState(false);
  const [showSessionSwitcher, setShowSessionSwitcher] = useState(false);
  const [showLoopTaskDialog, setShowLoopTaskDialog] = useState(false);
  const [showCronTaskDialog, setShowCronTaskDialog] = useState(false);
  const [showApproveModeDialog, setShowApproveModeDialog] = useState(false);
  const [showModelSwitcherDialog, setShowModelSwitcherDialog] = useState(false);
  const [showMemoryPanel, setShowMemoryPanel] = useState(false);
  const [showToolsSkillsPanel, setShowToolsSkillsPanel] = useState(false);

  // Get loop/cron task state
  const loopTask = useChatStore((s) => s.loopTask);
  const cronTasks = useChatStore((s) => s.cronTasks);
  const stopLoopTask = useChatStore((s) => s.stopLoopTask);
  const stopCronTask = useChatStore((s) => s.stopCronTask);

  // Global exception handling (P2 optimization - panic hook recovery)
  useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    const setupPanicListener = async () => {
      unlistenFn = await listen('backend-panic', (event) => {
        console.error('Backend panic:', event.payload);

        // Show error toast (using toast from component level)
        toast.addToast({
          type: 'error',
          message: '后端异常，正在恢复应用状态...',
          duration: 5000
        });

        // Re-initialize app state
        createSession();
        clearMessages();
        setCurrentView('chat');

        // Close all panels
        setShowPerformanceMonitor(false);
        setShowLspPanel(false);
        setShowCodeGraphPanel(false);
        setShowMcpPanel(false);
        setShowCommandBar(false);
        setShowShortcutHelp(false);
        setShowSessionSwitcher(false);
        setShowLoopTaskDialog(false);
        setShowCronTaskDialog(false);
        setShowApproveModeDialog(false);
        setShowModelSwitcherDialog(false);
        setShowMemoryPanel(false);
        setShowToolsSkillsPanel(false);
      });
    };

    setupPanicListener();

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, [createSession, clearMessages, toast]);

  // Global keyboard shortcuts (matching TUI key handling)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Global Esc: close all dialogs (highest priority)
      if (e.key === 'Escape' && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
        if (showModelSwitcherDialog) {
          e.preventDefault();
          setShowModelSwitcherDialog(false);
          return;
        }
        if (showApproveModeDialog) {
          e.preventDefault();
          setShowApproveModeDialog(false);
          return;
        }
        if (showMemoryPanel) {
          e.preventDefault();
          setShowMemoryPanel(false);
          return;
        }
        if (showToolsSkillsPanel) {
          e.preventDefault();
          setShowToolsSkillsPanel(false);
          return;
        }
        if (showCronTaskDialog) {
          e.preventDefault();
          setShowCronTaskDialog(false);
          return;
        }
        if (showLoopTaskDialog) {
          e.preventDefault();
          setShowLoopTaskDialog(false);
          return;
        }
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
      if (showCommandBar || showShortcutHelp || showSessionSwitcher || showLoopTaskDialog || showCronTaskDialog || showApproveModeDialog || showModelSwitcherDialog || showMemoryPanel || showToolsSkillsPanel) {
        return;
      }

      // VSCode-style shortcuts for quick actions (Ctrl+Shift variants)
      // These shortcuts work when QuickActionPanel has code input
      // Ctrl/Cmd + Shift + E: Explain code
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'E') {
        e.preventDefault();
        console.log('Quick action: Explain (use QuickActionPanel)');
        // Note: User should input code in QuickActionPanel first
      }
      // Ctrl/Cmd + Shift + F: Fix code
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'F') {
        e.preventDefault();
        console.log('Quick action: Fix (use QuickActionPanel)');
      }
      // Ctrl/Cmd + Shift + T: Generate tests
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'T') {
        e.preventDefault();
        console.log('Quick action: Generate tests (use QuickActionPanel)');
      }
      // Ctrl/Cmd + Shift + R: Refactor
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'R') {
        e.preventDefault();
        console.log('Quick action: Refactor (use QuickActionPanel)');
      }

      // Cmd/Ctrl + N: New chat
      if ((e.metaKey || e.ctrlKey) && e.key === 'n') {
        e.preventDefault();
        createSession();
        clearMessages();
        setCurrentView('chat');
      }
      // Cmd/Ctrl + T: Tasks view
      if ((e.metaKey || e.ctrlKey) && e.key === 't' && !e.shiftKey) {
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

      // Quick action shortcuts (matching VSCode extension)
      // Alt+E: Explain selected code (requires code input)
      if (e.altKey && e.key === 'e') {
        e.preventDefault();
        // This would trigger the explain action in QuickActionPanel
        console.log('Quick action: Explain (use QuickActionPanel or add code input first)');
        // Note: User should add code to QuickActionPanel first, then use this shortcut
      }
      // Alt+F: Fix selected code
      if (e.altKey && e.key === 'f') {
        e.preventDefault();
        console.log('Quick action: Fix');
      }
      // Alt+T: Generate tests
      if (e.altKey && e.key === 't' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        console.log('Quick action: Generate tests');
      }
      // Alt+R: Refactor
      if (e.altKey && e.key === 'r') {
        e.preventDefault();
        console.log('Quick action: Refactor');
      }
      // Alt+I: Improve
      if (e.altKey && e.key === 'i') {
        e.preventDefault();
        console.log('Quick action: Improve');
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
      // Alt+T: Toggle thinking collapse (matching TUI)
      // Note: This is handled in ChatView state, we just log here
      if (e.altKey && e.key === 't') {
        e.preventDefault();
        console.log('Alt+T: Toggle thinking collapse (handled in ChatView)');
      }
      // Alt+Up/Down: Fine scrolling (1 line) - matching TUI
      // Note: These are handled in ChatView, we log here for reference
      if (e.altKey && e.key === 'ArrowUp') {
        e.preventDefault();
        console.log('Alt+Up: Scroll up 1 line (handled in ChatView)');
      }
      if (e.altKey && e.key === 'ArrowDown') {
        e.preventDefault();
        console.log('Alt+Down: Scroll down 1 line (handled in ChatView)');
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
      // Alt+M: Approve mode dialog
      if (e.altKey && e.key === 'm') {
        e.preventDefault();
        setShowApproveModeDialog(prev => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [createSession, clearMessages, showCommandBar, showShortcutHelp, showSessionSwitcher, showLoopTaskDialog, showCronTaskDialog, showApproveModeDialog, showModelSwitcherDialog, showMemoryPanel, showToolsSkillsPanel, showLspPanel, showCodeGraphPanel, showMcpPanel, showPerformanceMonitor]);

  const renderView = () => {
    switch (currentView) {
      case 'chat':
        return <ChatView />;
      case 'tasks':
        return (
          <Suspense fallback={<LoadingFallback message="加载任务视图..." />}>
            <TaskViewLazy />
          </Suspense>
        );
      case 'settings':
        return (
          <Suspense fallback={<LoadingFallback message="加载设置面板..." />}>
            <SettingsPanel />
          </Suspense>
        );
      default:
        return <ChatView />;
    }
  };

  // Handle session switch
  const handleSessionSwitch = async (sessionId: string) => {
    await switchSession(sessionId);
    clearMessages();  // Clear current messages first
    // Load messages from the new session after switching
    await loadMessages();
    setCurrentView('chat');
  };

  return (
    <div className="flex h-screen bg-background text-foreground">
      <Sidebar currentView={currentView} onViewChange={setCurrentView} />
      <main className="flex-1 flex flex-col min-w-0">
        {renderView()}
      </main>

      {/* Status panels (overlay) */}
            {showLspPanel && (
              <div className="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 z-40">
                <Suspense fallback={<DialogSkeleton />}>
                  <LspStatusPanel onClose={() => setShowLspPanel(false)} />
                </Suspense>
              </div>
            )}
            {showCodeGraphPanel && (
              <div className="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 z-40">
                <Suspense fallback={<DialogSkeleton />}>
                  <CodeGraphStatusPanel onClose={() => setShowCodeGraphPanel(false)} />
                </Suspense>
              </div>
            )}
            {showMcpPanel && (
              <div className="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 z-40">
                <Suspense fallback={<DialogSkeleton />}>
                  <McpStatusPanel onClose={() => setShowMcpPanel(false)} />
                </Suspense>
              </div>
            )}
            {showPerformanceMonitor && (
              <div className="fixed bottom-4 right-4 z-40">
                <PerformanceMonitor />
              </div>
            )}

            {/* Session switcher */}
            {showSessionSwitcher && (
              <DialogErrorBoundary>
                <Suspense fallback={<DialogSkeleton />}>
                  <SessionSwitcherDialog
                    onClose={() => setShowSessionSwitcher(false)}
                    onSelectSession={handleSessionSwitch}
                    currentSessionId={currentSessionId}
                  />
                </Suspense>
              </DialogErrorBoundary>
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
                  } else if (cmd === '/loop') {
                    setShowLoopTaskDialog(true);
                  } else if (cmd === '/cron') {
                    setShowCronTaskDialog(true);
                  } else if (cmd === '/mode') {
                    setShowApproveModeDialog(true);
                  } else if (cmd === '/model') {
                    setShowModelSwitcherDialog(true);
                  } else if (cmd === '/memory') {
                    setShowMemoryPanel(true);
                  } else if (cmd === '/tools' || cmd === '/skills') {
                    setShowToolsSkillsPanel(true);
                  } else if (cmd === '/lsp') {
                    setShowLspPanel(true);
                  } else if (cmd === '/codegraph' || cmd === '/cg') {
                    setShowCodeGraphPanel(true);
                  } else if (cmd === '/mcp') {
                    setShowMcpPanel(true);
                  }
                }}
                onShowLoopDialog={() => setShowLoopTaskDialog(true)}
                onShowCronDialog={() => setShowCronTaskDialog(true)}
                onShowSessionSwitcher={() => setShowSessionSwitcher(true)}
                onShowMemoryPanel={() => setShowMemoryPanel(true)}
                onShowToolsSkillsPanel={() => setShowToolsSkillsPanel(true)}
                onClose={() => setShowCommandBar(false)}
              />
            )}

            {/* Shortcut help */}
            {showShortcutHelp && (
              <ShortcutHelp onClose={() => setShowShortcutHelp(false)} />
            )}

            {/* Approve mode dialog */}
            {showApproveModeDialog && (
              <DialogErrorBoundary>
                <Suspense fallback={<DialogSkeleton />}>
                  <ApproveModeDialog onClose={() => setShowApproveModeDialog(false)} />
                </Suspense>
              </DialogErrorBoundary>
            )}

            {/* Model switcher dialog */}
            {showModelSwitcherDialog && (
              <DialogErrorBoundary>
                <Suspense fallback={<DialogSkeleton />}>
                  <ModelSwitcherDialog onClose={() => setShowModelSwitcherDialog(false)} />
                </Suspense>
              </DialogErrorBoundary>
            )}

            {/* Loop task dialog */}
            {showLoopTaskDialog && (
              <DialogErrorBoundary>
                <Suspense fallback={<DialogSkeleton />}>
                  <LoopTaskDialog onClose={() => setShowLoopTaskDialog(false)} />
                </Suspense>
              </DialogErrorBoundary>
            )}

            {/* Cron task dialog */}
            {showCronTaskDialog && (
              <DialogErrorBoundary>
                <Suspense fallback={<DialogSkeleton />}>
                  <CronTaskDialog onClose={() => setShowCronTaskDialog(false)} />
                </Suspense>
              </DialogErrorBoundary>
            )}

            {/* Memory panel */}
            {showMemoryPanel && (
              <DialogErrorBoundary>
                <Suspense fallback={<DialogSkeleton />}>
                  <MemoryPanel onClose={() => setShowMemoryPanel(false)} />
                </Suspense>
              </DialogErrorBoundary>
            )}

            {/* Tools & Skills panel */}
            {showToolsSkillsPanel && (
              <DialogErrorBoundary>
                <Suspense fallback={<DialogSkeleton />}>
                  <ToolsSkillsPanel onClose={() => setShowToolsSkillsPanel(false)} />
                </Suspense>
              </DialogErrorBoundary>
            )}

            {/* Loop/Cron task indicator */}
            <LoopTaskIndicator
              loopTask={loopTask}
              cronTasks={cronTasks}
              onStopLoop={stopLoopTask}
              onStopCron={stopCronTask}
            />

            {/* Shortcut hint indicator */}
            <ShortcutHintIndicator onClick={() => setShowShortcutHelp(true)} />
          </div>
  );
}

// Outer wrapper component that provides context
function App() {
  return (
    <ToastProvider>
      <ServerStatusProvider>
        <ErrorBoundary>
          <AppContent />
        </ErrorBoundary>
      </ServerStatusProvider>
    </ToastProvider>
  );
}

export default App;