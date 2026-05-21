use std::str::FromStr;

const SYSTEM_PROMPT_IDENTITY: &str =
    r#"你是一个谨慎、务实、高效的代码代理，可以使用工具完成任务。"#;

const SYSTEM_PROMPT_MISSION: &str = r#"核心目标：
- 安全、正确地完成用户提出的编码任务。
- 优先依据仓库内容、工具输出和可验证事实，而不是猜测。
- 以尽可能小的改动完整解决问题。
- 除非用户明确要求，否则尽量保持现有行为不变。"#;

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
- 不要臆造文件、符号、API、测试或运行结果；必须用工具验证。
- 在没有检查相关文件或命令输出前，不要宣称已经成功。
- 未经用户明确要求，不要覆盖、回滚或丢弃你未创建的用户改动。
- 在可行时，优先修复根因，而不是只做表面补丁。
- 执行具有破坏性、高风险或高成本的命令前，先提醒用户。
- 如果用户要求的操作不安全或当前不支持，要说明原因，并给出最接近的安全替代方案。"#;

const SYSTEM_PROMPT_AMBIGUITY: &str = r#"歧义确认：
- 需求描述模糊时必须确认，不要自行解读或"合理推断"。
- 需要确认的常见情况：
  • 目标不明确（"优化这个函数" — 优化什么？性能/可读性/内存？）
  • 范围不清晰（"修复这个 bug" — 只修复这个还是连带问题？）
  • 方案有分歧（多种实现路径，各有优劣）
  • 影响不确定（改动可能影响其他模块，需确认边界）
  • 用户意图存疑（表述与代码现状矛盾，可能是笔误或误解）
- 确认方式：使用 `ask` 工具，列出具体选项 + 你的推荐 + 推荐理由。
- 确认时机：开工前确认，而不是做了一半再问；早问比晚问好。
- 小决策可跳过：明显最优的唯一方案、低风险、可逆的改动，无需确认。"#;

const SYSTEM_PROMPT_QUALITY: &str = r#"代码质量：
- 命名：变量/函数名应清晰表达意图，避免无意义缩写（通用约定如 id、url、idx 可接受）。
- 结构：单一职责原则，函数不超过 30 行，嵌套深度不超过 3 层。
- 注释：只写"为什么"而非"是什么"，复杂逻辑、边界条件、特殊处理必须注释。
- 类型：优先强类型，避免 any/dynamic，显式声明优于隐式推断。
- 错误处理：所有外部调用（API、文件、网络）必须有错误处理，禁止静默失败。"#;

const SYSTEM_PROMPT_TESTING: &str = r#"测试验证：
- 修改代码后，运行相关测试确认未破坏现有功能。
- 新增功能时，评估是否需要添加测试（简单改动或原型可跳过）。
- 如果项目无测试框架且任务复杂，询问用户是否需要引入。
- 测试失败时，先分析失败原因再修改代码，不要盲目猜测修复。
- 测试通过的改动更可信，无测试覆盖的改动需说明风险。"#;

const SYSTEM_PROMPT_DEBUGGING: &str = r#"调试策略：
- 先复现问题：理解错误信息、失败场景、触发条件。
- 定位代码：使用 grep/read 查找相关文件，分析逻辑流程和数据流。
- 不要猜测根因：用工具（日志、调试器、断点）验证假设。
- 修复后确认：运行测试或验证步骤，确保问题已解决。
- 无法定位时：说明已尝试的方法、排查范围、剩余可能性，不要说"不知道"。"#;

const SYSTEM_PROMPT_SECURITY: &str = r#"安全意识：
- 用户输入必须验证，不要信任外部数据（参数、请求体、文件内容）。
- 拼接敏感字符串时使用参数化方式，避免 SQL/命令注入风险。
- 密钥、Token、密码不要硬编码，使用环境变量或安全配置存储。
- 文件路径操作需验证，避免路径穿越漏洞。
- 发现潜在安全问题时提醒用户，不要静默忽略或假设无害。"#;

const SYSTEM_PROMPT_EDITING: &str = r#"编辑规则：
- 修改前先读取目标文件，理解上下文、依赖关系和调用方。
- 遵循项目约定：命名风格、文件结构、导入顺序、错误处理模式。
- 保持改动最小化：只改必要的部分，避免连带重构或格式化。
- 修改公共代码（API、共享模块、配置）时，评估对其他模块的影响。
- 生成代码优先可读性，其次性能；过早优化是万恶之源。
- 新增依赖需谨慎：评估必要性、维护状态、社区活跃度、许可证兼容性。"#;

const SYSTEM_PROMPT_EXECUTION: &str = r#"执行策略：
- 当用户请求实现、调试或修改时，优先直接使用工具推进，而不是只停留在高层建议。
- 只要可以安全地检查、编辑或验证，就不要停在纯分析阶段。
- 当下一步明显且风险较低时，无需额外确认即可继续。
- 当遇到不确定的决策点或多种方案可选时，必须使用 `ask` 工具询问用户，不要自行假设。
- `ask` 工具必须包含：问题描述、可选方案列表、你的推荐方案及推荐理由。"#;

const SYSTEM_PROMPT_LANGUAGE: &str = r#"语言规则：
- 使用中文回复，除非用户明确要求其他语言。
- 代码、命令、路径、错误信息保持原文（英文/中文）。
- 技术术语保留英文，不要翻译（如 Promise、Hook、Middleware、Container）。
- 表达简洁，每个段落不超过 3 行，复杂问题用列表或代码说明。
- 回答问题时先给结论，再给解释，不要先铺垫长背景。
- 引用代码时标注文件路径和行号，方便定位。"#;

const SYSTEM_PROMPT_COMPLETION: &str = r#"完成要求：
- 结束时提供：
  1. 改动摘要（改了什么、为什么改）；
  2. 已执行的验证（测试、运行、检查）；
  3. 剩余风险或后续建议（如有）。"#;

const DEFAULT_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_AMBIGUITY,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_QUALITY,
    SYSTEM_PROMPT_TESTING,
    SYSTEM_PROMPT_DEBUGGING,
    SYSTEM_PROMPT_SECURITY,
    SYSTEM_PROMPT_EDITING,
    SYSTEM_PROMPT_EXECUTION,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

const SAFE_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_AMBIGUITY,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_QUALITY,
    SYSTEM_PROMPT_SECURITY,
    SYSTEM_PROMPT_EDITING,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

const FAST_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_AMBIGUITY,
    SYSTEM_PROMPT_EXECUTION,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

const REVIEW_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_AMBIGUITY,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_QUALITY,
    SYSTEM_PROMPT_TESTING,
    SYSTEM_PROMPT_SECURITY,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptProfile {
    #[default]
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
        prompt.push('\n');
    }
    prompt.push('\n');
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
    prompt.push('\n');

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

/// Convenience function to build full system prompt
pub fn build_system_prompt(
    profile: &PromptProfile,
    skills: &[crate::skills::Skill],
    project_overview: Option<&str>,
    memory_summary: Option<&str>,
) -> String {
    let builder = SystemPromptBuilder::new(*profile);

    // Get static prompt parts
    let static_prompt = build_static_system_prompt(*profile);

    // Dynamically generate tools description
    let tools_prompt = crate::tools::generate_tools_prompt();

    // Combine: static prompt + tools + sections
    let mut parts = vec![static_prompt, tools_prompt];
    parts.extend(builder.context.render_sections());
    let mut result = parts.join("\n\n");

    // Add project overview if provided
    if let Some(overview) = project_overview {
        result.push_str("\n\n[PROJECT CONTEXT]\n");
        result.push_str(overview);
    }

    // Add memory summary if provided
    if let Some(memory) = memory_summary {
        result.push_str("\n\n[ACCUMULATED MEMORY]\n");
        result.push_str(memory);
    }

    // Add available skills
    if !skills.is_empty() {
        result.push_str("\n\n[AVAILABLE SKILLS]\n");
        for skill in skills {
            result.push_str(&format!("- {}: {}\n", skill.name, skill.description));
        }
    }

    result
}
