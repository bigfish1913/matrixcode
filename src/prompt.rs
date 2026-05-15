use std::str::FromStr;

const SYSTEM_PROMPT_IDENTITY: &str = r#"你是一个谨慎、务实、高效的代码代理，可以使用工具完成任务。"#;

const SYSTEM_PROMPT_MISSION: &str = r#"核心目标：
- 安全、正确地完成用户提出的编码任务。
- 优先依据仓库内容、工具输出和可验证事实，而不是猜测。
- 以尽可能小的改动完整解决问题。
- 除非用户明确要求，否则尽量保持现有行为不变。"#;

const SYSTEM_PROMPT_TOOLS: &str = r#"可用工具：
- read / write / edit / multi_edit：文件读写。修改已有文件前先 read；修改已有文件时优先使用 edit 或 multi_edit，而不是 write。
- ls：列出目录下的一级内容（非递归）。
- glob：按模式查找文件。
- search：按正则搜索文件内容。
- bash：执行构建、测试、lint、git 检查等 shell 命令。
- todo_write：用于维护非简单任务的待办列表；始终保持且仅保持一个 in_progress。
- websearch：客户端网页搜索工具，使用 DuckDuckGo 搜索并返回结果列表。
- web_search：服务端网页搜索工具（仅 Anthropic），由 API 直接执行搜索，结果更精准。
- webfetch：获取指定 URL 的页面内容。
- skill：当任务匹配某项技能时，优先加载技能说明，而不是自行猜测。

工具选择建议：
- 需要搜索网页信息时，优先使用 web_search（服务端搜索，结果更精准）。
- 如果 web_search 不可用或需要更多控制，可使用 websearch（客户端搜索）。
- 要获取具体网页内容时，使用 webfetch。"#;

const SYSTEM_PROMPT_WORKFLOW: &str = r#"工作方式：
1. 先理解需求，再查看相关代码和文件。
2. 对于非简单任务，使用 todo_write 创建并持续更新待办列表。
3. 每次调用工具前，先简短说明接下来要做什么。
4. 优先基于证据做判断；如果不确定，就继续检查。
5. 保持改动聚焦、最小，并与现有代码风格一致。
6. 除非为安全完成任务所必需，否则避免无关重构。
7. 修改完成后，执行最小且相关的验证。
8. 如果无法验证，要明确说明原因和剩余风险。"#;

const SYSTEM_PROMPT_BEHAVIOR: &str = r#"行为约束：
- 如果需求存在歧义，且该歧义会阻碍安全推进，先使用 `ask` 工具向用户提问澄清。
- 当存在多种可行方案时，不要猜测或自选；使用 `ask` 工具列出选项、提供推荐方案及理由，等待用户决定。
- 不要臆造文件、符号、API、测试或运行结果；必须用工具验证。
- 在没有检查相关文件或命令输出前，不要宣称已经成功。
- 未经用户明确要求，不要覆盖、回滚或丢弃你未创建的用户改动。
- 在可行时，优先修复根因，而不是只做表面补丁。
- 执行具有破坏性、高风险或高成本的命令前，先提醒用户。
- 如果用户要求的操作不安全或当前不支持，要说明原因，并给出最接近的安全替代方案。"#;

const SYSTEM_PROMPT_EDITING: &str = r#"编辑规则：
- 修改文件前，先读取目标文件或相关片段。
- 遵循周边代码的命名、格式和架构约定。
- 除非任务明确要求，否则不要修改生成文件。
- 除非确有必要，否则不要新增依赖。
- 尽量只改动完成任务所需的最少文件。"#;

const SYSTEM_PROMPT_EXECUTION: &str = r#"执行策略：
- 当用户请求实现、调试或修改时，优先直接使用工具推进，而不是只停留在高层建议。
- 只要可以安全地检查、编辑或验证，就不要停在纯分析阶段。
- 当下一步明显且风险较低时，无需额外确认即可继续。
- 当遇到不确定的决策点或多种方案可选时，必须使用 `ask` 工具询问用户，不要自行假设。
- `ask` 工具必须包含：问题描述、可选方案列表、你的推荐方案及推荐理由。"#;

const SYSTEM_PROMPT_LANGUAGE: &str = r#"语言规则：
- 默认使用中文回复，除非用户明确要求使用其他语言。
- 代码、命令、路径、报错信息和标识符在合适时保持原文。
- 表达应简洁、清晰、面向执行。"#;

const SYSTEM_PROMPT_COMPLETION: &str = r#"完成要求：
- 结束时提供：
  1. 改动摘要；
  2. 已执行的验证；
  3. 剩余风险或后续建议。"#;

const DEFAULT_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_TOOLS,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_EDITING,
    SYSTEM_PROMPT_EXECUTION,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

const SAFE_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_TOOLS,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_EDITING,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

const FAST_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_TOOLS,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_EXECUTION,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

const REVIEW_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_TOOLS,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptProfile {
    Default,
    Safe,
    Fast,
    Review,
}

impl PromptProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Safe => "safe",
            Self::Fast => "fast",
            Self::Review => "review",
        }
    }

    const fn static_modules(self) -> &'static [&'static str] {
        match self {
            Self::Default => DEFAULT_SYSTEM_PROMPT_MODULES,
            Self::Safe => SAFE_SYSTEM_PROMPT_MODULES,
            Self::Fast => FAST_SYSTEM_PROMPT_MODULES,
            Self::Review => REVIEW_SYSTEM_PROMPT_MODULES,
        }
    }
}

impl Default for PromptProfile {
    fn default() -> Self {
        Self::Default
    }
}

impl FromStr for PromptProfile {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "default" => Ok(Self::Default),
            "safe" => Ok(Self::Safe),
            "fast" => Ok(Self::Fast),
            "review" => Ok(Self::Review),
            other => Err(format!(
                "unknown prompt profile '{other}'. expected one of: default, safe, fast, review"
            )),
        }
    }
}

pub fn build_static_system_prompt(profile: PromptProfile) -> String {
    profile.static_modules().join("\n\n")
}

pub const SECTION_PROJECT_CONTEXT: &str = "PROJECT CONTEXT";
pub const SECTION_TASK_CONTEXT: &str = "TASK CONTEXT";
pub const SECTION_AVAILABLE_SKILLS: &str = "AVAILABLE SKILLS";
pub const SECTION_ACCUMULATED_MEMORY: &str = "ACCUMULATED MEMORY";

/// Memory summary section header for system prompt.
pub const MEMORY_SUMMARY_HEADER: &str = r#"【跨会话记忆摘要】
以下是从过往对话中积累的关键知识，请在回答时参考这些信息以保持一致性："#;

/// Memory entry format template.
pub const MEMORY_ENTRY_TEMPLATE: &str = "{icon} {category}: {content}";

// =============================================================================
// Overview Generation Prompt Constants
// =============================================================================

const OVERVIEW_PROMPT_HEADER: &str = "请分析以下项目并生成一份详细的项目概览文档 MATRIX.md。\n\n";

const OVERVIEW_PROMPT_REQUIREMENTS: &[&str] = &[
    "1. 分析项目的架构和核心功能",
    "2. 说明关键目录的作用",
    "3. 提供常用开发命令（构建、测试、运行等）",
    "4. 总结项目的关键模式和约定",
    "5. 提供开发注意事项",
    "6. 如果有业务逻辑（如订单流程、用户系统等），请详细说明",
];

const OVERVIEW_PROMPT_FORMAT: &str = "输出格式：直接输出 markdown 内容，不要加代码块包裹。";

const OVERVIEW_PROMPT_FOOTER: &str = "请基于以上信息，生成一份详细的项目概览文档 MATRIX.md。";

/// Project context for overview generation.
pub struct OverviewContext {
    pub project_name: String,
    pub project_type: String,
    pub directory_structure: String,
    pub config_files: Vec<(String, String)>,
    pub readme: Option<String>,
    pub source_files: Vec<(String, String)>,
}

/// Build the AI prompt for generating project overview (MATRIX.md).
pub fn build_overview_prompt(context: &OverviewContext) -> String {
    let mut prompt = String::new();

    prompt.push_str(OVERVIEW_PROMPT_HEADER);
    prompt.push_str("要求：\n");
    for req in OVERVIEW_PROMPT_REQUIREMENTS {
        prompt.push_str(req);
        prompt.push_str("\n");
    }
    prompt.push_str("\n");
    prompt.push_str(OVERVIEW_PROMPT_FORMAT);
    prompt.push_str("\n\n---\n\n");

    // Add project info
    prompt.push_str(&format!("项目名称: {}\n", context.project_name));
    prompt.push_str(&format!("项目类型: {}\n\n", context.project_type));

    // Add directory structure
    prompt.push_str("## 目录结构\n\n");
    prompt.push_str("```\n");
    prompt.push_str(&context.directory_structure);
    prompt.push_str("```\n\n");

    // Add config files
    if !context.config_files.is_empty() {
        prompt.push_str("## 配置文件\n\n");
        for (filename, content) in &context.config_files {
            prompt.push_str(&format!("### {}\n\n", filename));
            prompt.push_str("```\n");
            prompt.push_str(content);
            prompt.push_str("\n```\n\n");
        }
    }

    // Add README
    if let Some(readme) = &context.readme {
        prompt.push_str("## README.md (开头部分)\n\n");
        prompt.push_str(readme);
        prompt.push_str("\n\n");
    }

    // Add key source files
    if !context.source_files.is_empty() {
        prompt.push_str("## 关键源文件\n\n");
        for (filename, content) in &context.source_files {
            prompt.push_str(&format!("### {}\n\n", filename));
            prompt.push_str("```\n");
            prompt.push_str(content);
            prompt.push_str("\n```\n\n");
        }
    }

    prompt.push_str("---\n\n");
    prompt.push_str(OVERVIEW_PROMPT_FOOTER);
    prompt.push_str("\n");

    prompt
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSection {
    title: String,
    body: String,
}

impl PromptSection {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Option<Self> {
        let title = title.into().trim().to_string();
        let body = body.into().trim().to_string();
        if title.is_empty() || body.is_empty() {
            return None;
        }
        Some(Self { title, body })
    }

    pub fn render(&self) -> String {
        format!("[{}]\n{}", self.title, self.body)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptContext {
    sections: Vec<PromptSection>,
}

impl PromptContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_section(&mut self, title: impl Into<String>, body: impl Into<String>) {
        if let Some(section) = PromptSection::new(title, body) {
            self.sections.push(section);
        }
    }

    pub fn with_section(mut self, title: impl Into<String>, body: impl Into<String>) -> Self {
        self.push_section(title, body);
        self
    }

    pub fn push_available_skills(&mut self, body: impl Into<String>) {
        self.push_section(SECTION_AVAILABLE_SKILLS, body);
    }

    pub fn with_available_skills(mut self, body: impl Into<String>) -> Self {
        self.push_available_skills(body);
        self
    }

    pub fn extend(&mut self, other: PromptContext) {
        self.sections.extend(other.sections);
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn render_sections(&self) -> Vec<String> {
        self.sections.iter().map(PromptSection::render).collect()
    }
}

#[derive(Debug, Clone)]
pub struct SystemPromptBuilder {
    profile: PromptProfile,
    context: PromptContext,
}

impl SystemPromptBuilder {
    pub fn new(profile: PromptProfile) -> Self {
        Self {
            profile,
            context: PromptContext::new(),
        }
    }

    pub fn push_section(&mut self, title: impl Into<String>, body: impl Into<String>) {
        self.context.push_section(title, body);
    }

    pub fn with_section(mut self, title: impl Into<String>, body: impl Into<String>) -> Self {
        self.push_section(title, body);
        self
    }

    pub fn push_context(&mut self, context: PromptContext) {
        self.context.extend(context);
    }

    pub fn with_context(mut self, context: PromptContext) -> Self {
        self.push_context(context);
        self
    }

    pub fn push_available_skills(&mut self, body: impl Into<String>) {
        self.context.push_available_skills(body);
    }

    pub fn with_available_skills(mut self, body: impl Into<String>) -> Self {
        self.push_available_skills(body);
        self
    }

    pub fn build(&self) -> String {
        let mut parts = vec![build_static_system_prompt(self.profile)];
        parts.extend(self.context.render_sections());
        parts.join("\n\n")
    }
}
