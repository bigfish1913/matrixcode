import React, { useState, useEffect, useCallback } from 'react';

// Notification types matching TUI feedback
type NotificationType = 'success' | 'error' | 'warning' | 'info' | 'loading';

// Notification configuration
interface NotificationConfig {
  id: string;
  type: NotificationType;
  title: string;
  message?: string;
  duration?: number;  // milliseconds (0 = no auto-close)
  position?: 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
  action?: {
    label: string;
    onClick: () => void;
  };
  dismissible?: boolean;
  progress?: number;  // 0-100 for loading type
}

// Notification icons and colors
const NOTIFICATION_CONFIG: Record<NotificationType, { icon: string; color: string; bgColor: string }> = {
  success: { icon: '✓', color: 'text-green-500', bgColor: 'bg-green-500/10 border-green-500' },
  error: { icon: '✗', color: 'text-red-500', bgColor: 'bg-red-500/10 border-red-500' },
  warning: { icon: '⚠', color: 'text-yellow-500', bgColor: 'bg-yellow-500/10 border-yellow-500' },
  info: { icon: 'ℹ', color: 'text-blue-500', bgColor: 'bg-blue-500/10 border-blue-500' },
  loading: { icon: '⏳', color: 'text-primary', bgColor: 'bg-primary/10 border-primary' },
};

// Global notification state
let notificationId = 0;
const notificationListeners: Set<(notifications: NotificationConfig[]) => void> = new Set();
let currentNotifications: NotificationConfig[] = [];

// Add notification
export function showNotification(
  type: NotificationType,
  title: string,
  options?: Partial<NotificationConfig>
): string {
  const id = `notif-${++notificationId}`;
  const notification: NotificationConfig = {
    id,
    type,
    title,
    duration: type === 'loading' ? 0 : (options?.duration ?? 5000),
    position: options?.position ?? 'bottom-right',
    dismissible: options?.dismissible ?? true,
    ...options,
  };

  currentNotifications = [...currentNotifications, notification];
  notificationListeners.forEach(listener => listener(currentNotifications));

  // Auto dismiss (if duration > 0)
  if (notification.duration && notification.duration > 0) {
    setTimeout(() => {
      dismissNotification(id);
    }, notification.duration);
  }

  return id;
}

// Update notification (for loading type)
export function updateNotification(
  id: string,
  updates: Partial<NotificationConfig>
): void {
  currentNotifications = currentNotifications.map(n =>
    n.id === id ? { ...n, ...updates } : n
  );
  notificationListeners.forEach(listener => listener(currentNotifications));
}

// Dismiss notification
export function dismissNotification(id: string): void {
  currentNotifications = currentNotifications.filter(n => n.id !== id);
  notificationListeners.forEach(listener => listener(currentNotifications));
}

// Clear all notifications
export function clearAllNotifications(): void {
  currentNotifications = [];
  notificationListeners.forEach(listener => listener(currentNotifications));
}

// Convenience functions
export const notify = {
  success: (title: string, message?: string) => showNotification('success', title, { message }),
  error: (title: string, message?: string) => showNotification('error', title, { message, duration: 8000 }),
  warning: (title: string, message?: string) => showNotification('warning', title, { message }),
  info: (title: string, message?: string) => showNotification('info', title, { message }),
  loading: (title: string, message?: string) => showNotification('loading', title, { message, duration: 0 }),
  progress: (title: string, progress: number, message?: string) =>
    showNotification('loading', title, { message, progress, duration: 0 }),
};

// Notification container component
export function NotificationContainer() {
  const [notifications, setNotifications] = useState<NotificationConfig[]>([]);

  useEffect(() => {
    const listener = (notifs: NotificationConfig[]) => {
      setNotifications(notifs);
    };
    notificationListeners.add(listener);
    return () => {
      notificationListeners.delete(listener);
    };
  }, []);

  // Group by position
  const groupedNotifications = notifications.reduce((acc, notif) => {
    const pos = notif.position || 'bottom-right';
    if (!acc[pos]) acc[pos] = [];
    acc[pos].push(notif);
    return acc;
  }, {} as Record<string, NotificationConfig[]>);

  if (notifications.length === 0) return null;

  return (
    <>
      {Object.entries(groupedNotifications).map(([position, notifs]) => (
        <div
          key={position}
          className={`fixed z-50 flex flex-col gap-2 max-w-sm ${
            position === 'top-left' ? 'top-4 left-4' :
            position === 'top-right' ? 'top-4 right-4' :
            position === 'bottom-left' ? 'bottom-4 left-4' :
            'bottom-4 right-4'
          }`}
        >
          {notifs.map((notif) => (
            <NotificationItem key={notif.id} notification={notif} />
          ))}
        </div>
      ))}
    </>
  );
}

// Single notification item
function NotificationItem({ notification }: { notification: NotificationConfig }) {
  const [isVisible, setIsVisible] = useState(false);
  const config = NOTIFICATION_CONFIG[notification.type];

  useEffect(() => {
    // Animate in
    setIsVisible(true);
  }, []);

  const handleDismiss = () => {
    setIsVisible(false);
    setTimeout(() => {
      dismissNotification(notification.id);
    }, 200);
  };

  return (
    <div
      className={`border rounded-lg p-3 shadow-lg ${config.bgColor} transition-all ${
        isVisible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2'
      }`}
      role="alert"
      aria-live="polite"
    >
      {/* Header */}
      <div className="flex items-start gap-3">
        {/* Icon */}
        <span className={`text-xl ${config.color}`}>
          {config.icon}
        </span>

        {/* Content */}
        <div className="flex-1">
          <div className={`font-medium text-sm ${config.color}`}>
            {notification.title}
          </div>
          {notification.message && (
            <div className="text-xs text-muted-foreground mt-1">
              {notification.message}
            </div>
          )}

          {/* Progress bar (for loading) */}
          {notification.type === 'loading' && notification.progress !== undefined && (
            <div className="mt-2">
              <div className="h-1.5 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary transition-all"
                  style={{ width: `${notification.progress}%` }}
                />
              </div>
              <div className="text-xs text-muted-foreground mt-1">
                {notification.progress}% complete
              </div>
            </div>
          )}
        </div>

        {/* Dismiss button */}
        {notification.dismissible && (
          <button
            onClick={handleDismiss}
            className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            aria-label="Dismiss notification"
          >
            ✕
          </button>
        )}
      </div>

      {/* Action button */}
      {notification.action && (
        <div className="mt-2">
          <button
            onClick={() => {
              if (notification.action) {
                notification.action.onClick();
                handleDismiss();
              }
            }}
            className="px-3 py-1.5 bg-primary text-primary-foreground rounded text-xs hover:bg-primary/90 transition-colors"
          >
            {notification.action.label}
          </button>
        </div>
      )}

      {/* Timer indicator */}
      {notification.duration && notification.duration > 0 && (
        <div className="mt-2">
          <div
            className="h-0.5 bg-muted rounded-full overflow-hidden"
            style={{
              animation: `shrink ${notification.duration}ms linear forwards`,
            }}
          />
        </div>
      )}
    </div>
  );
}

// CSS animation for timer
const NOTIFICATION_CSS = `
@keyframes shrink {
  from { width: 100%; }
  to { width: 0%; }
}
`;

// Hook for components to show notifications
export function useNotification() {
  return {
    show: showNotification,
    update: updateNotification,
    dismiss: dismissNotification,
    clearAll: clearAllNotifications,
    notify,
  };
}