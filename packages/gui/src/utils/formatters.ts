/**
 * Shared formatting utilities
 * Eliminates code duplication across components
 */

/**
 * Format token count with K/M suffixes
 * @param count - Token count
 * @returns Formatted string (e.g., "1.5k", "2.3m")
 */
export function formatTokenCount(count: number): string {
  if (count >= 1000000) return `${(count / 1000000).toFixed(1)}m`;
  if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
  return String(count);
}

/**
 * Format timestamp to readable time
 * @param timestamp - Unix timestamp in milliseconds
 * @returns Time string (e.g., "14:30")
 */
export function formatTime(timestamp?: number): string {
  if (!timestamp) return '';
  const date = new Date(timestamp);
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

/**
 * Format execution time in ms/s/m format
 * @param ms - Execution time in milliseconds
 * @returns Formatted string (e.g., "45ms", "1.23s", "2m 15s")
 */
export function formatExecutionTime(ms?: number): string {
  if (!ms) return '';
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`;
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  return `${mins}m ${secs}s`;
}

/**
 * Format elapsed time for activity display
 * @param seconds - Elapsed seconds
 * @returns Formatted string (e.g., "45s", "2:30")
 */
export function formatElapsed(seconds: number): string {
  if (seconds < 60) return `${Math.floor(seconds)}s`;
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

/**
 * Format message count with localization
 * @param count - Message count
 * @returns Formatted string (e.g., "42 messages")
 */
export function formatMessageCount(count: number): string {
  return `${count} message${count !== 1 ? 's' : ''}`;
}

/**
 * Format percentage for performance display
 * @param value - Percentage value (0-100)
 * @returns Formatted string (e.g., "45%")
 */
export function formatPercentage(value: number): string {
  return `${Math.round(value)}%`;
}