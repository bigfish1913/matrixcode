import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

// Version information
interface VersionInfo {
  current: string;
  latest: string;
  hasUpdate: boolean;
  releaseDate?: string;
  releaseNotes?: string;
  downloadUrl?: string;
}

// Update check result
interface UpdateCheckResult {
  available: boolean;
  version?: string;
  notes?: string;
  url?: string;
}

// Load current version
function getCurrentVersion(): string {
  // Try to get from Tauri
  try {
    // This will be called from Tauri backend
    return '0.4.48';  // Fallback
  } catch {
    return '0.4.48';
  }
}

// Check for updates (mock implementation)
async function checkForUpdates(): Promise<UpdateCheckResult> {
  try {
    // Try Tauri backend first
    const result = await invoke<UpdateCheckResult>('check_updates');
    return result;
  } catch {
    // Mock fallback for development
    const currentVersion = getCurrentVersion();
    const latestVersion = '0.4.49';  // Mock latest

    return {
      available: latestVersion !== currentVersion,
      version: latestVersion,
      notes: 'Bug fixes and performance improvements',
      url: 'https://github.com/bigfish1913/matrixcode/releases/latest',
    };
  }
}

// Global update state
let updateInfo: VersionInfo | null = null;
const updateListeners: Set<(info: VersionInfo | null) => void> = new Set();

// Check update and notify listeners
export async function performUpdateCheck(): Promise<VersionInfo | null> {
  const current = getCurrentVersion();
  const result = await checkForUpdates();

  updateInfo = {
    current,
    latest: result.version || current,
    hasUpdate: result.available,
    releaseNotes: result.notes,
    downloadUrl: result.url,
  };

  updateListeners.forEach(listener => listener(updateInfo));
  return updateInfo;
}

// Get cached update info
export function getUpdateInfo(): VersionInfo | null {
  return updateInfo;
}

// Update notification dialog
interface UpdateNotificationDialogProps {
  onClose: () => void;
  onUpdate?: () => void;
}

export function UpdateNotificationDialog({ onClose, onUpdate }: UpdateNotificationDialogProps) {
  const [info, setInfo] = useState<VersionInfo | null>(updateInfo);
  const [checking, setChecking] = useState(false);
  const [downloading, setDownloading] = useState(false);

  useEffect(() => {
    const listener = (newInfo: VersionInfo | null) => {
      setInfo(newInfo);
    };
    updateListeners.add(listener);
    return () => {
      updateListeners.delete(listener);
    };
  }, []);

  // Manual check
  const handleCheck = async () => {
    setChecking(true);
    try {
      await performUpdateCheck();
    } finally {
      setChecking(false);
    }
  };

  // Download/update
  const handleUpdate = async () => {
    if (!info?.downloadUrl) return;

    setDownloading(true);
    try {
      // Open download URL in browser
      window.open(info.downloadUrl, '_blank');
      onUpdate?.();
    } finally {
      setDownloading(false);
    }
  };

  // Skip this version
  const handleSkip = () => {
    // Save skipped version
    localStorage.setItem('matrixcode-skip-version', info?.latest || '');
    onClose();
  };

  // Check if already skipped
  const skippedVersion = localStorage.getItem('matrixcode-skip-version');
  const shouldShow = info?.hasUpdate && info.latest !== skippedVersion;

  if (!shouldShow) {
    return null;
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b bg-green-500/10">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2 text-green-500">
              <span>🎉</span>
              <span>New Version Available</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
        </div>

        {/* Version info */}
        <div className="p-4">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-4">
              <div className="text-center">
                <div className="text-xs text-muted-foreground">Current</div>
                <div className="font-mono font-medium">{info?.current}</div>
              </div>
              <div className="text-2xl text-muted-foreground">→</div>
              <div className="text-center">
                <div className="text-xs text-muted-foreground">Latest</div>
                <div className="font-mono font-medium text-green-500">{info?.latest}</div>
              </div>
            </div>
          </div>

          {/* Release notes */}
          {info?.releaseNotes && (
            <div className="bg-muted/30 rounded p-3 mb-4">
              <div className="text-xs font-medium mb-1">Release Notes:</div>
              <p className="text-sm text-muted-foreground">
                {info.releaseNotes}
              </p>
            </div>
          )}

          {/* What's new */}
          <div className="text-sm text-muted-foreground">
            <div className="font-medium mb-2">What's New:</div>
            <ul className="space-y-1 text-xs">
              <li className="flex items-center gap-2">
                <span className="text-green-500">✓</span>
                <span>Bug fixes and improvements</span>
              </li>
              <li className="flex items-center gap-2">
                <span className="text-green-500">✓</span>
                <span>Performance optimizations</span>
              </li>
              <li className="flex items-center gap-2">
                <span className="text-green-500">✓</span>
                <span>New features and enhancements</span>
              </li>
            </ul>
          </div>
        </div>

        {/* Actions */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex gap-2">
            <button
              onClick={handleSkip}
              className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
            >
              Skip This Version
            </button>
            <button
              onClick={handleCheck}
              disabled={checking}
              className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent disabled:opacity-50 transition-colors"
            >
              {checking ? 'Checking...' : 'Check Again'}
            </button>
            <button
              onClick={handleUpdate}
              disabled={downloading}
              className="flex-1 px-4 py-2 bg-green-500 text-white rounded-lg text-sm hover:bg-green-500/90 disabled:opacity-50 transition-colors"
            >
              {downloading ? 'Downloading...' : 'Update Now'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// Update checker hook
export function useUpdateChecker(autoCheck: boolean = true) {
  const [info, setInfo] = useState<VersionInfo | null>(null);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    const listener = (newInfo: VersionInfo | null) => {
      setInfo(newInfo);
    };
    updateListeners.add(listener);
    return () => {
      updateListeners.delete(listener);
    };
  }, []);

  // Auto check on mount
  useEffect(() => {
    if (autoCheck) {
      const lastCheck = localStorage.getItem('matrixcode-last-update-check');
      const now = Date.now();
      const oneDay = 24 * 60 * 60 * 1000;

      // Only check if last check was more than 24 hours ago
      if (!lastCheck || now - parseInt(lastCheck) > oneDay) {
        performUpdateCheck();
        localStorage.setItem('matrixcode-last-update-check', String(now));
      }
    }
  }, [autoCheck]);

  const checkNow = async () => {
    setChecking(true);
    try {
      const result = await performUpdateCheck();
      setInfo(result);
      localStorage.setItem('matrixcode-last-update-check', String(Date.now()));
    } finally {
      setChecking(false);
    }
  };

  return {
    info,
    checking,
    checkNow,
    hasUpdate: info?.hasUpdate ?? false,
    currentVersion: info?.current ?? getCurrentVersion(),
    latestVersion: info?.latest ?? getCurrentVersion(),
  };
}

// Version badge component
export function VersionBadge() {
  const { currentVersion, hasUpdate } = useUpdateChecker(false);

  return (
    <span className="text-xs text-muted-foreground font-mono">
      v{currentVersion}
      {hasUpdate && (
        <span className="ml-1 text-green-500 animate-pulse">
          (new available)
        </span>
      )}
    </span>
  );
}