import React, { createContext, useContext, useState, ReactNode } from 'react';

// Toast notification context for global use
interface ToastMessage {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  message: string;
  duration?: number;
}

interface ToastContextValue {
  addToast: (toast: Omit<ToastMessage, 'id'>) => void;
  removeToast: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToastContext() {
  const context = useContext(ToastContext);
  if (!context) {
    // Return fallback if not in provider context
    return {
      addToast: (toast: Omit<ToastMessage, 'id'>) => {
        console.log('Toast (no provider):', toast);
      },
      removeToast: () => {},
    };
  }
  return context;
}

// Provider that manages toast state
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const idCounter = React.useRef(0);

  const addToast = (toast: Omit<ToastMessage, 'id'>) => {
    const id = `toast-${++idCounter.current}`;
    setToasts(prev => [...prev, { ...toast, id }]);

    // Auto remove after duration
    const duration = toast.duration || 3000;
    setTimeout(() => {
      removeToast(id);
    }, duration);
  };

  const removeToast = (id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  };

  return (
    <ToastContext.Provider value={{ addToast, removeToast }}>
      {children}
      {/* Toast container */}
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </ToastContext.Provider>
  );
}

// Toast container component
function ToastContainer({ toasts, onRemove }: {
  toasts: ToastMessage[];
  onRemove: (id: string) => void;
}) {
  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-4 right-4 z-50 space-y-2 max-w-sm">
      {toasts.map(toast => (
        <ToastItem key={toast.id} toast={toast} onRemove={onRemove} />
      ))}
    </div>
  );
}

// Individual toast item
function ToastItem({ toast, onRemove }: {
  toast: ToastMessage;
  onRemove: (id: string) => void;
}) {
  const typeStyles = {
    info: 'bg-blue-500/10 border-blue-500/30 text-blue-600',
    success: 'bg-green-500/10 border-green-500/30 text-green-600',
    warning: 'bg-yellow-500/10 border-yellow-500/30 text-yellow-600',
    error: 'bg-red-500/10 border-red-500/30 text-red-600',
  };

  const typeIcons = {
    info: 'ℹ️',
    success: '✅',
    warning: '⚠️',
    error: '❌',
  };

  return (
    <div
      className={`border rounded-lg p-3 shadow-lg animate-slide-in-right ${typeStyles[toast.type]}`}
    >
      <div className="flex items-start gap-2">
        <span className="text-lg">{typeIcons[toast.type]}</span>
        <div className="flex-1">
          <p className="text-sm font-medium">{toast.message}</p>
        </div>
        <button
          onClick={() => onRemove(toast.id)}
          className="text-muted-foreground hover:text-foreground text-xs"
        >
          ✕
        </button>
      </div>
    </div>
  );
}