/**
 * Type guards for event data validation
 * Improves type safety in chatStore event handlers
 */

import type {
  ToolUseData,
  ToolUseInputData,
  ToolResultData,
  ErrorData,
  UsageData,
  ProgressData,
  TextData,
  ThinkingData,
} from '../stores/chatStore';

/**
 * Check if data is valid TextData
 */
export function isTextData(data: unknown): data is { text: TextData } {
  return typeof data === 'object' &&
         data !== null &&
         'text' in data &&
         typeof (data as any).text === 'object' &&
         'delta' in (data as any).text;
}

/**
 * Check if data is valid ThinkingData
 */
export function isThinkingData(data: unknown): data is { thinking: ThinkingData } {
  return typeof data === 'object' &&
         data !== null &&
         'thinking' in data &&
         typeof (data as any).thinking === 'object' &&
         'delta' in (data as any).thinking;
}

/**
 * Check if data is valid ToolUseData (nested format)
 */
export function isNestedToolUseData(data: unknown): data is { tool_use: ToolUseData } {
  return typeof data === 'object' &&
         data !== null &&
         'tool_use' in data &&
         typeof (data as any).tool_use === 'object' &&
         'id' in (data as any).tool_use &&
         'name' in (data as any).tool_use &&
         typeof (data as any).tool_use.id === 'string' &&
         typeof (data as any).tool_use.name === 'string';
}

/**
 * Check if data is valid ToolUseData (flat format)
 */
export function isFlatToolUseData(data: unknown): data is ToolUseData {
  return typeof data === 'object' &&
         data !== null &&
         'id' in data &&
         'name' in data &&
         typeof (data as any).id === 'string' &&
         typeof (data as any).name === 'string';
}

/**
 * Extract ToolUseData from either nested or flat format
 */
export function extractToolUseData(data: unknown): ToolUseData | null {
  if (isNestedToolUseData(data)) {
    return data.tool_use;
  }
  if (isFlatToolUseData(data)) {
    return data;
  }
  return null;
}

/**
 * Check if data is valid ToolUseInputData
 */
export function isToolUseInputData(data: unknown): data is { tool_use_input: ToolUseInputData } {
  return typeof data === 'object' &&
         data !== null &&
         'tool_use_input' in data &&
         typeof (data as any).tool_use_input === 'object' &&
         'id' in (data as any).tool_use_input &&
         'delta' in (data as any).tool_use_input;
}

/**
 * Check if data is valid ToolResultData (nested format)
 */
export function isNestedToolResultData(data: unknown): data is { tool_result: ToolResultData } {
  return typeof data === 'object' &&
         data !== null &&
         'tool_result' in data &&
         typeof (data as any).tool_result === 'object' &&
         'tool_use_id' in (data as any).tool_result &&
         'content' in (data as any).tool_result;
}

/**
 * Check if data is valid ToolResultData (flat format)
 */
export function isFlatToolResultData(data: unknown): data is ToolResultData {
  return typeof data === 'object' &&
         data !== null &&
         'tool_use_id' in data &&
         'content' in data &&
         typeof (data as any).tool_use_id === 'string' &&
         typeof (data as any).content === 'string';
}

/**
 * Extract ToolResultData from either nested or flat format
 */
export function extractToolResultData(data: unknown): ToolResultData | null {
  if (isNestedToolResultData(data)) {
    return data.tool_result;
  }
  if (isFlatToolResultData(data)) {
    return data;
  }
  return null;
}

/**
 * Check if data is valid ErrorData
 */
export function isErrorData(data: unknown): data is { error: ErrorData } {
  return typeof data === 'object' &&
         data !== null &&
         'error' in data &&
         typeof (data as any).error === 'object' &&
         'message' in (data as any).error;
}

/**
 * Check if data is valid UsageData
 */
export function isUsageData(data: unknown): data is { usage: UsageData } {
  return typeof data === 'object' &&
         data !== null &&
         'usage' in data &&
         typeof (data as any).usage === 'object' &&
         'input_tokens' in (data as any).usage &&
         'output_tokens' in (data as any).usage;
}

/**
 * Check if data is valid ProgressData
 */
export function isProgressData(data: unknown): data is { progress: ProgressData } {
  return typeof data === 'object' &&
         data !== null &&
         'progress' in data &&
         typeof (data as any).progress === 'object' &&
         'message' in (data as any).progress;
}

/**
 * Check if data has a specific field
 */
export function hasField<T extends string>(data: unknown, field: T): data is Record<T, unknown> {
  return typeof data === 'object' && data !== null && field in data;
}