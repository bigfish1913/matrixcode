import React, { useState, useRef, useEffect } from 'react';

// Toast notification system (matching TUI system message display)
interface ToastMessage {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  message: string;
  duration?: number;  // ms, default 3000
}

interface ToastContextType {
  toasts: ToastMessage[];
  addToast: (toast: Omit<ToastMessage, 'id'>) => void;
  removeToast: (id: string) => void;
}

// Toast container component
export function ToastContainer({ toasts, onRemove }: {
  toasts: ToastMessage[];
  onRemove: (id: string) => void;
}) {
  return (
    <div className="fixed top-4 right-4 z-50 space-y-2 max-w-sm">
      {toasts.map((toast) => (
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
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    // Animate in
    setIsVisible(true);

    // Auto remove after duration
    const timer = setTimeout(() => {
      setIsVisible(false);
      setTimeout(() => onRemove(toast.id), 300);  // Wait for fade out
    }, toast.duration || 3000);

    return () => clearTimeout(timer);
  }, [toast.id, toast.duration, onRemove]);

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
      className={`border rounded-lg p-3 shadow-lg transition-all duration-300 ${
        typeStyles[toast.type]
      } ${isVisible ? 'animate-slide-in-right opacity-100' : 'animate-fade-out opacity-0'}`}
    >
      <div className="flex items-start gap-2">
        <span className="text-lg">{typeIcons[toast.type]}</span>
        <div className="flex-1">
          <p className="text-sm font-medium">{toast.message}</p>
        </div>
        <button
          onClick={() => {
            setIsVisible(false);
            setTimeout(() => onRemove(toast.id), 300);
          }}
          className="text-muted-foreground hover:text-foreground text-xs"
        >
          ✕
        </button>
      </div>
    </div>
  );
}

// Toast hook for easy usage
export function useToast() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const idRef = useRef(0);

  const addToast = (toast: Omit<ToastMessage, 'id'>) => {
    const id = `toast-${++idRef.current}`;
    setToasts(prev => [...prev, { ...toast, id }]);
  };

  const removeToast = (id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  };

  return {
    toasts,
    addToast,
    removeToast,
    ToastContainer: () => (
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    ),
  };
}

// Notification badge component (for status bar)
export function NotificationBadge({
  count,
  type = 'info',
}: {
  count: number;
  type?: 'info' | 'warning' | 'error';
}) {
  if (count === 0) return null;

  const typeColors = {
    info: 'bg-blue-500',
    warning: 'bg-yellow-500',
    error: 'bg-red-500',
  };

  return (
    <span className={`px-1.5 py-0.5 text-xs rounded-full ${typeColors[type]} text-white font-medium`}>
      {count > 99 ? '99+' : count}
    </span>
  );
}

// Notification dot (for subtle indication)
export function NotificationDot({ type = 'info' }: { type?: 'info' | 'warning' | 'error' }) {
  const typeColors = {
    info: 'bg-blue-500',
    warning: 'bg-yellow-500',
    error: 'bg-red-500',
  };

  return (
    <span className={`w-2 h-2 rounded-full ${typeColors[type]} animate-pulse`} />
  );
}

// System message display (matching TUI push_message for system alerts)
export function SystemMessage({ message, type }: {
  message: string;
  type?: 'info' | 'warning' | 'error';
}) {
  const typeStyles = {
    info: 'bg-blue-500/5 border-blue-500/20 text-blue-600',
    warning: 'bg-yellow-500/5 border-yellow-500/20 text-yellow-600',
    error: 'bg-red-500/5 border-red-500/20 text-red-600',
  };

  const typeIcons = {
    info: 'ℹ️',
    warning: '⚠️',
    error: '❌',
  };

  return (
    <div className={`border rounded-lg p-2 mb-2 text-xs animate-slide-in-up ${typeStyles[type || 'info']}`}>
      <div className="flex items-center gap-2">
        <span>{typeIcons[type || 'info']}</span>
        <span className="font-medium">{message}</span>
      </div>
    </div>
  );
}