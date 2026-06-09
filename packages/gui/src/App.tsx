import { useState } from 'react';
import { Sidebar, type ViewType } from './components/Sidebar';
import { ChatView } from './components/ChatView';
import { TaskView } from './components/TaskView';
import { SettingsPanel } from './components/SettingsPanel';

function App() {
  const [currentView, setCurrentView] = useState<ViewType>('chat');

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
