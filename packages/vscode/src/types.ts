/**
 * MatrixCode Event Types
 * Mirrors the Rust AgentEvent protocol
 */

export interface AgentEvent {
  event_type: EventType;
  timestamp: number;
  data?: EventData;
}

export type EventType =
  | 'session_started'
  | 'session_ended'
  | 'text_start'
  | 'text_delta'
  | 'text_end'
  | 'thinking_start'
  | 'thinking_delta'
  | 'thinking_end'
  | 'tool_use_start'
  | 'tool_use_input_delta'
  | 'tool_use_input_end'
  | 'tool_result'
  | 'error'
  | 'usage'
  | 'progress'
  | 'compression_triggered'
  | 'compression_completed'
  | 'memory_loaded';

export interface EventData {
  text?: { delta: string };
  thinking?: { delta: string; signature?: string };
  tool_use?: { id: string; name: string; input?: unknown };
  tool_use_input?: { id: string; delta: string };
  tool_result?: { tool_use_id: string; content: string; is_error: boolean };
  error?: { message: string; code?: string; source?: string };
  usage?: {
    input_tokens: number;
    output_tokens: number;
    cache_creation_input_tokens?: number;
    cache_read_input_tokens?: number;
  };
  progress?: { message: string; percentage?: number };
  compression?: { original_tokens: number; compressed_tokens: number; ratio: number };
  memory?: { summary: string; entries_count: number };
}

export interface DaemonRequest {
  type: 'chat' | 'quick_action' | 'status' | 'new_session';
  content?: string;
  action?: string;
  context?: RequestContext;
}

export interface RequestContext {
  workspace?: string;
  file?: string;
  language?: string;
  selection?: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
}