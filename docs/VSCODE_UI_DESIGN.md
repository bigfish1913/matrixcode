# MatrixCode VSCode 扩展交互设计

## 参考：主流 AI 编程助手的交互方式

### 1. Cursor / Claude Code 风格

#### 侧边栏聊天
- 位置：左侧活动栏图标 → 聊天面板
- 特点：
  - 分割视图：上半部分聊天，下半部分代码预览/差异
  - 支持多轮对话
  - 显示 tool use 状态
  - 代码块可一键应用/复制

#### 内联建议
- Cmd+K (Ctrl+K) 触发内联编辑
- 在编辑器中直接显示建议
- 差异对比视图（绿色添加，红色删除）
- Accept/Reject 按钮

#### 快捷操作
- 选中文本 → 右键菜单 → AI 操作
- Explain、Fix、Refactor、Generate Tests
- 操作结果显示在聊天面板

#### 状态反馈
- 处理时显示 spinner
- 状态栏显示当前状态
- Tool use 实时显示

---

## MatrixCode 改进方案

### 1. 聊天面板改进

#### 当前问题
- 简单的 HTML webview
- 缺少代码差异预览
- Tool use 显示不够直观

#### 改进方案
```typescript
// 聊天面板布局
<div class="chat-container">
  <!-- 上半部分：对话历史 -->
  <div class="messages-area">
    <message role="user">...</message>
    <message role="assistant">
      <thinking-block>...</thinking-block>
      <tool-use-block name="read" status="running">
        📖 Reading file...
      </tool-use-block>
      <code-block language="typescript">
        <diff-viewer>
          <original-code>...</original-code>
          <suggested-code>...</suggested-code>
          <action-buttons>
            <button>Apply</button>
            <button>Copy</button>
            <button>Reject</button>
          </action-buttons>
        </diff-viewer>
      </code-block>
    </message>
  </div>
  
  <!-- 下半部分：代码预览 -->
  <div class="code-preview-area">
    <tabs>
      <tab>Diff View</tab>
      <tab>Applied Changes</tab>
    </tabs>
    <diff-editor>
      左侧：原始代码
      右侧：建议代码
    </diff-editor>
  </div>
  
  <!-- 输入区域 -->
  <div class="input-area">
    <textarea placeholder="Ask MatrixCode..."></textarea>
    <buttons>
      <button>Send</button>
      <button>Attach File</button>
      <button>Quick Action</button>
    </buttons>
  </div>
</div>
```

### 2. 内联编辑 (Cmd+K / Ctrl+K)

#### 触发方式
- 选中文本 → Cmd+K
- 或右键 → "MatrixCode: Inline Edit"

#### UI 流程
```
1. 用户选中文本
2. 按 Cmd+K → 弹出输入框："What changes do you want?"
3. 输入指令（如 "Add error handling"）
4. AI 生成建议 → 显示差异视图
5. 用户选择：
   - Accept: 应用更改
   - Reject: 拒绝更改
   - Edit: 手动调整
```

#### 实现方案
```typescript
// InlineEditProvider.ts
export class InlineEditProvider {
  async provideInlineEdit(
    document: TextDocument,
    range: Range,
    instruction: string
  ): Promise<InlineEdit> {
    // 发送到 CLI
    const response = await client.inlineEdit({
      file: document.uri.fsPath,
      range: range,
      instruction: instruction
    });
    
    // 返回差异
    return {
      originalText: document.getText(range),
      suggestedText: response.code,
      diff: computeDiff(originalText, suggestedText)
    };
  }
}
```

### 3. 快捷操作改进

#### 当前
- 命令面板触发
- 结果显示在聊天面板

#### 改进
```
选中文本 → 右键 → MatrixCode 菜单：
├─ 📖 Explain
├─ 🔧 Fix Errors
├─ 🧪 Generate Tests
├─ 🔨 Refactor
│  ├─ Extract Function
│  ├─ Extract Variable
│  ├─ Rename
│  └─ Optimize
├─ 📝 Add Documentation
├─ 🌐 Translate to...
└─ 💬 Ask MatrixCode...
```

#### 结果显示
- 小弹窗显示简要结果
- "View Details" 按钮 → 聊天面板
- 代码块 → 一键应用

### 4. Tool Use 可视化

#### 当前问题
- Tool use 只在日志中显示
- 用户看不到正在执行的操作

#### 改进
```typescript
// ToolUseProgress.ts
export class ToolUseProgress {
  private statusBar: StatusBarItem;
  
  showToolUse(tool: string, status: 'running' | 'done' | 'error') {
    const icons = {
      read: '📖',
      write: '📝',
      edit: '✏️',
      bash: '⚡',
      search: '🔍',
    };
    
    const statusIcons = {
      running: '⏳',
      done: '✅',
      error: '❌',
    };
    
    this.statusBar.text = `${icons[tool]} ${tool} ${statusIcons[status]}`;
    this.statusBar.show();
  }
}
```

#### Tool Use 卡片（聊天面板）
```html
<div class="tool-use-card">
  <header>
    <icon>📖</icon>
    <title>read</title>
    <status>running</status>
    <spinner></spinner>
  </header>
  <content>
    <input>
      path: "src/main.rs"
    </input>
    <output>
      Loading file content...
    </output>
  </content>
</div>
```

### 5. 代码差异预览

#### 实现
```typescript
// DiffViewer.ts
export class DiffViewer {
  async showDiff(
    original: string,
    suggested: string,
    language: string
  ): Promise<void> {
    // 创建差异编辑器
    const diffEditor = vscode.window.createDiffEditor();
    
    diffEditor.setDocuments(
      vscode.Uri.parse('original'),
      vscode.Uri.parse('suggested')
    );
    
    // 添加操作按钮
    const actions = [
      { label: 'Apply', action: () => this.applyDiff() },
      { label: 'Copy', action: () => this.copyCode() },
      { label: 'Reject', action: () => this.closeDiff() },
    ];
    
    // 显示在聊天面板的代码预览区域
    chatView.showCodePreview(diffEditor, actions);
  }
  
  async applyDiff(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    editor.edit(editBuilder => {
      editBuilder.replace(selection, suggestedText);
    });
  }
}
```

### 6. 状态反馈

#### 状态栏
```
正常状态: 🤖 MatrixCode Ready
处理中: 🤖 MatrixCode ⏳ Thinking...
Tool Use: 🤖 MatrixCode 📖 read ⏳
完成: 🤖 MatrixCode ✅ Done
错误: 🤖 MatrixCode ❌ Error
```

#### 进度指示器
```typescript
export class ProgressIndicator {
  async showProgress(message: string): Promise<void> {
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: "MatrixCode",
        cancellable: true
      },
      (progress, token) => {
        progress.report({ message: message });
        // ...
      }
    );
  }
}
```

---

## 实现优先级

### Phase 1: 基础改进（立即实现）
1. ✅ 聊天面板美化
2. ✅ Tool Use 可视化
3. ✅ 状态反馈优化
4. ✅ 快捷操作菜单

### Phase 2: 核心功能（本周）
5. 内联编辑 (Cmd+K)
6. 代码差异预览
7. 一键应用更改
8. 多文件支持

### Phase 3: 高级功能（下周）
9. 上下文自动提取
10. 项目理解
11. 会话持久化
12. 配置同步

---

## UI 设计稿

### 聊天面板
```
┌─────────────────────────────────────┐
│ 🤖 MatrixCode                 ⚙️ 🗑️ │
├─────────────────────────────────────┤
│                                     │
│ 👤 User:                            │
│    Fix this error                   │
│                                     │
│ 🤖 Assistant:                       │
│    ⏳ Thinking...                   │
│                                     │
│    📖 read src/main.rs ⏳           │
│    └────────────────────            │
│    | path: "src/main.rs" |          │
│    └────────────────────            │
│                                     │
│    I found the issue. The function  │
│    is missing error handling.       │
│                                     │
│    ```typescript                    │
│    // Diff Preview                  │
│    - function process() {           │
│    + function process() {           │
│    +   try {                        │
│          // existing code           │
│    +   } catch (e) {                │
│    +     handleError(e);            │
│    +   }                            │
│    }                                │
│    ```                              │
│                                     │
│    [Apply] [Copy] [Reject]          │
│                                     │
├─────────────────────────────────────┤
│ Code Preview (Diff View)            │
│ ┌───────────┬───────────┐          │
│ │ Original  │ Suggested │          │
│ │           │           │          │
│ │ function  │ function  │          │
│ │ process() │ process() │          │
│ │ {         │ { try {   │          │
│ │   ...     │   ...     │          │
│ │ }         │ } catch } │          │
│ └───────────┴───────────┘          │
├─────────────────────────────────────┤
│ 💬 Ask MatrixCode...        [Send] │
│ [📎] [⚡] [⚙️]                     │
└─────────────────────────────────────┘
```

### 内联编辑
```
Editor:
┌─────────────────────────────────────┐
│ function processData(data) {        │
│   return data.map(item => {         │
│     ██ selected text ████████       │
│     return item.value;              │
│   });                               │
│ }                                   │
│                                     │
│ ┌─ Inline Edit Popup ─────────────┐│
│ │ 💬 What changes?                ││
│ │ [Add error handling___________] ││
│ │                                 ││
│ │ 🤖 Suggestion:                  ││
│ │ - return item.value;            ││
│ │ + try {                         ││
│ │ +   return item.value;          ││
│ │ + } catch (e) {                 ││
│ │ +   console.error(e);           ││
│ │ +   return null;                ││
│ │ + }                             ││
│ │                                 ││
│ │ [Accept] [Reject] [Edit]        ││
│ └───────────────────────���─────────┘│
└─────────────────────────────────────┘
```

### 快捷菜单
```
Right-click on selection:
┌──────────────────────┐
│ MatrixCode           │
│ ├─ 📖 Explain        │
│ ├─ 🔧 Fix Errors     │
│ ├─ 🧪 Generate Tests │
│ ├─ 🔨 Refactor       │
│ │  ├─ Extract Func   │
│ │  ├─ Extract Var    │
│ │  └─ Optimize       │
│ ├─ 📝 Add Docs       │
│ └─ 💬 Ask...         │
└──────────────────────┘
```

---

## 下一步行动

1. **立即改进聊天面板 UI** - 更美观、更功能完整
2. **添加 Tool Use 可视化** - 实时显示操作状态
3. **实现内联编辑** - Cmd+K 快捷键
4. **添加代码差异预览** - 一键应用更改
5. **优化快捷菜单** - 右键菜单改进