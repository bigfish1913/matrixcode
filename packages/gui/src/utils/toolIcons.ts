// Tool icon mapping matching TUI (draw/messages.rs)
export const TOOL_ICONS: Record<string, string> = {
  read: '📖',
  write: '📝',
  edit: '✏️',
  multi_edit: '✏️',
  bash: '⚡',
  search: '🔍',
  grep: '🔍',
  glob: '🔍',
  ls: '🔍',
  websearch: '🌐',
  webfetch: '🔗',
  ask: '❓',
  agent: '🤖',
  workflow: '📊',
  skill: '⚡',
  cron: '⏰',
  todo: '📋',
  memory: '💾',
  mcp: '🔌',
  codegraph: '📊',
  notebook: '📓',
  notebook_edit: '✏️',
  design: '🎨',
  plan: '📝',
  verify: '✅',
  code_review: '👀',
  simplify: '🧹',
  run: '▶️',
  init: '🚀',
  loop: '🔄',
  claude_api: '📡',
  update_config: '⚙️',
  keybindings_help: '⌨️',
  fewer_permission_prompts: '🔒',
  deep_research: '🔬',
  frontend_design: '🎨',
  systematic_debugging: '🐛',
  test_driven_development: '🧪',
  writing_plans: '📝',
  verification_before_completion: '✅',
  receiving_code_review: '👀',
  requesting_code_review: '👀',
  finishing_a_development_branch: '🏁',
  executing_plans: '🎯',
  dispatching_parallel_agents: '🔀',
  subagent_driven_development: '🤖',
  using_git_worktrees: '🌳',
  writing_skills: '✍️',
  using_superpowers: '⚡',
  brainstorming: '💭',
  // Default fallback
  default: '🔧',
};

// Activity type icons (from TuiApp activity handling)
export const ACTIVITY_ICONS: Record<string, string> = {
  idle: '⏸',
  thinking: '💭',
  reading: '📖',
  writing: '📝',
  editing: '✏️',
  searching: '🔍',
  running: '⚡',
  websearch: '🌐',
  webfetch: '🔗',
  tool: '🔧',
  asking: '❓',
};

// Activity status colors (matching TUI)
export const ACTIVITY_COLORS: Record<string, string> = {
  idle: 'text-muted-foreground',
  thinking: 'text-purple-500',
  reading: 'text-cyan-500',
  writing: 'text-yellow-500',
  editing: 'text-yellow-500',
  searching: 'text-cyan-500',
  running: 'text-red-500',
  websearch: 'text-blue-500',
  webfetch: 'text-blue-500',
  tool: 'text-cyan-500',
  asking: 'text-red-500',
};

// Spinner animation frames (matching TUI ANIM_MS = 80ms)
export const SPINNER_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

// Approve mode colors (matching TUI)
export const APPROVE_MODE_COLORS: Record<string, string> = {
  ask: 'bg-gray-400/20 text-gray-500',
  auto: 'bg-green-500/20 text-green-600',
  strict: 'bg-red-500/20 text-red-600',
};

// Get tool icon by name
export function getToolIcon(toolName: string): string {
  const normalized = toolName.toLowerCase().replace(/_/g, '_');
  return TOOL_ICONS[normalized] || TOOL_ICONS.default;
}

// Format tool name for display
export function formatToolName(toolName: string): string {
  // Convert snake_case to display format
  return toolName
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (l) => l.toUpperCase());
}

// Get spinner frame by index
export function getSpinnerFrame(index: number): string {
  return SPINNER_FRAMES[index % SPINNER_FRAMES.length];
}