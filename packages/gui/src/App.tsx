import { useState, useEffect } from 'react';
import { Sidebar, type ViewType } from './components/Sidebar';
import { ChatView } from './components/ChatView';
import { TaskView } from './components/TaskView';
import { SettingsPanel } from './components/SettingsPanel';
import { useSessionStore } from './stores/sessionStore';
import { useChatStore } from './stores/chatStore';

function App() {
  const [currentView, setCurrentView] = useState<ViewType>('chat');
  const createSession = useSessionStore((s) => s.createSession);
  const clearMessages = useChatStore((s) => s.clearMessages);

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
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
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [createSession, clearMessages]);

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

  return (
    <div className="flex h-screen bg-background text-foreground">
      <Sidebar currentView={currentView} onViewChange={setCurrentView} />
      <main className="flex-1 flex flex-col min-w-0">
        {renderView()}
      </main>
    </div>
  );
}

export default App;
