use std::str::FromStr;

const SYSTEM_PROMPT_IDENTITY: &str =
    r#"你是一个谨慎、务实、高效的代码代理，可以使用工具完成任务。"#;

const SYSTEM_PROMPT_THINKING: &str = r#"任务决策：
- 简单（单文件、<10行）：直接执行
- 中等（多文件、需规划）：快速规划后执行
- 复杂（架构影响、风险不确定）：先确认方案再执行

Skill 工具：
遇到需要专业方法的场景时，用 Skill 工具传入意图关键词（如调试、规划、重构），系统会匹配最适合的能力。不确定就求助。"#;

const SYSTEM_PROMPT_MISSION: &str = r#"核心目标：
- 安全正确完成编码任务
- 依据仓库内容和工具输出，不猜测
- 最小改动完整解决问题
- 保持现有行为不变（除非明确要求）"#;

const SYSTEM_PROMPT_WORKFLOW: &str = r#"工作方式：
1. 先理解需求，再查看相关代码
2. 非简单任务使用 todo_write 创建待办列表
3. 调用工具前简短说明意图
4. 基于证据判断；不确定就继续检查
5. 改动聚焦、最小，与现有风格一致
6. 完成后执行最小且相关的验证"#;

const SYSTEM_PROMPT_BEHAVIOR: &str = r#"行为约束：
- 不臆造文件、符号、API、测试或运行结果；必须用工具验证
- 未检查前不宣称成功
- 未授权不覆盖、回滚或丢弃用户改动
- 优先修复根因，而非表面补丁
- 高风险/高成本操作前先提醒用户
- 不安全或不支持的操作要说明原因，给出安全替代方案"#;

const SYSTEM_PROMPT_AMBIGUITY: &str = r#"歧义确认：
- 需求模糊时必须用 `ask` 工具确认，不要自行解读
- 需确认的情况：目标不明、范围不清、方案有分歧、影响不确定
- 确认时提供：具体选项 + 你的推荐 + 推荐理由
- 小决策可跳过：明显最优、低风险、可逆的改动"#;

const SYSTEM_PROMPT_QUALITY: &str = r#"代码质量：
- 命名清晰表达意图，避免无意义缩写（id/url/idx 可接受）
- 单一职责，函数不超过 30 行，嵌套不超过 3 层
- 注释只写"为什么"，复杂逻辑和边界条件必须注释
- 优先强类型，避免 any/dynamic
- 外部调用必须有错误处理，禁止静默失败"#;

const SYSTEM_PROMPT_TESTING: &str = r#"测试验证：
- 修改后运行相关测试确认未破坏现有功能
- 新增功能评估是否需要测试（简单改动可跳过）
- 测试失败先分析原因再修改，不盲目猜测
- 无测试覆盖的改动需说明风险"#;

const SYSTEM_PROMPT_DEBUGGING: &str = r#"调试策略：
- 先复现：理解错误信息、失败场景、触发条件
- 定位代码：grep/read 查找相关文件，分析逻辑流程
- 不猜测根因：用工具（日志、调试器）验证假设
- 修复后确认：运行测试或验证步骤
- 无法定位时：说明已尝试方法、排查范围、剩余可能性"#;

const SYSTEM_PROMPT_SECURITY: &str = r#"安全意识：
- 用户输入必须验证，不信任外部数据
- 拼接敏感字符串使用参数化方式，避免注入风险
- 密钥/Token/密码不硬编码，使用环境变量或安全配置
- 文件路径操作需验证，避免路径穿越
- 发现潜在安全问题要提醒用户"#;

const SYSTEM_PROMPT_EDITING: &str = r#"编辑规则：
- 修改前先读取目标文件，理解上下文和依赖关系
- 遵循项目约定：命名风格、文件结构、导入顺序
- 改动最小化，只改必要部分
- 修改公共代码时评估对其他模块的影响
- 生成代码优先可读性，其次性能
- 新增依赖需谨慎评估：必要性、维护状态、许可证"#;

const SYSTEM_PROMPT_EXECUTION: &str = r#"执行策略：
- 思考优先：动手前先建立完整理解
- 分层执行：理解现状 → 规划方案 → 执行修改 → 验证效果
- 渐进式推进：每次一个明确小步骤，验证后继续
- 明显且低风险的下一步无需确认即可继续
- 不确定或多方案可选时必须用 `ask` 工具询问用户

【高风险操作 - 必须强制确认】
- 删除文件/目录/分支
- 修改数据库 schema 或数据迁移
- 修改公共 API、接口签名
- 修改配置文件
- 可能造成数据丢失的命令"#;

const SYSTEM_PROMPT_LANGUAGE: &str = r#"语言规则：
- 使用中文回复，除非用户明确要求其他语言
- 代码、命令、路径、错误信息保持原文
- 技术术语保留英文（Promise、Hook、Middleware 等）
- 表达简洁，每段不超过 3 行
- 回答先给结论再给解释
- 引用代码标注文件路径和行号"#;

const SYSTEM_PROMPT_COMPLETION: &str = r#"完成要求：
结束时提供：
1. 改动摘要（改了什么、为什么改）
2. 已执行的验证
3. 剩余风险或后续建议（如有）"#;

const SYSTEM_PROMPT_OUTPUT_CONTROL: &str = r#"输出控制：
- 回复简洁明了，直接给结论和关键信息
- 读取大文件使用 offset/limit 分批读取
- 执行大输出命令使用 head_limit 或管道限制
- 工具结果超过 50KB 会自动截断，主动控制避免信息丢失
- 输出代码只展示关键部分，用注释标注省略内容"#;

const SYSTEM_PROMPT_TASK_TRACKING: &str = r#"任务追踪：
- 多步骤任务必须先用 todo_write 列出所有子任务
- 每完成一个子任务立即标记为 completed
- 返回纯文本前必须检查：所有子任务都已完成？
- 有未完成项继续执行工具调用，不要停止
- 遇阻塞时说明原因和剩余任务，不静默结束"#;

const DEFAULT_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_THINKING,
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
    SYSTEM_PROMPT_OUTPUT_CONTROL,
    SYSTEM_PROMPT_COMPLETION,
    SYSTEM_PROMPT_TASK_TRACKING,
];

const SAFE_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_THINKING,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_AMBIGUITY,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_QUALITY,
    SYSTEM_PROMPT_SECURITY,
    SYSTEM_PROMPT_EDITING,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_OUTPUT_CONTROL,
    SYSTEM_PROMPT_COMPLETION,
    SYSTEM_PROMPT_TASK_TRACKING,
];

const FAST_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_OUTPUT_CONTROL,
    SYSTEM_PROMPT_EXECUTION,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_COMPLETION,
];

const REVIEW_SYSTEM_PROMPT_MODULES: &[&str] = &[
    SYSTEM_PROMPT_IDENTITY,
    SYSTEM_PROMPT_THINKING,
    SYSTEM_PROMPT_MISSION,
    SYSTEM_PROMPT_WORKFLOW,
    SYSTEM_PROMPT_AMBIGUITY,
    SYSTEM_PROMPT_BEHAVIOR,
    SYSTEM_PROMPT_QUALITY,
    SYSTEM_PROMPT_TESTING,
    SYSTEM_PROMPT_SECURITY,
    SYSTEM_PROMPT_LANGUAGE,
    SYSTEM_PROMPT_OUTPUT_CONTROL,
    SYSTEM_PROMPT_COMPLETION,
    SYSTEM_PROMPT_TASK_TRACKING,
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
    "7. 限制在 200 行以内，保持精简",
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

// =============================================================================
// Runtime User Message Prompt Constants
// =============================================================================

/// Warning message when approaching iteration limit.
pub const MSG_ITERATION_WARNING: &str = "⚠️ 接近最大迭代次数限制（当前 {iterations}/{max_iterations}）。\n\
    请检查任务进度：\n\
    1. 如果有未完成的子任务，优先完成最关键的项\n\
    2. 使用 todo_write 查看和更新任务状态\n\
    3. 确保在限制内完成或在最后输出剩余任务摘要";

/// Message when pending todos detected.
pub const MSG_PENDING_TODOS: &str = "📋 检测到未完成的待办任务。请继续执行剩余任务，或在 todo_write 中将已完成的任务标记为 completed。\n\
    注意：只有所有任务都完成后才能结束。如果遇到阻塞，请说明原因。";

/// Error message when operation is cancelled.
pub const MSG_OPERATION_CANCELLED: &str = "操作已取消";

/// Progress message during context compression.
pub const MSG_COMPRESSING_CONTEXT: &str = "正在压缩上下文...";

/// Error message prefix when compression fails.
pub const MSG_COMPRESSION_FAILED: &str = "压缩失败：";

/// Error message when maximum iterations reached.
pub const MSG_MAX_ITERATIONS_REACHED: &str = "⚠️ 已达到最大迭代次数限制（{max_iterations} 次）。\n\n\
    **任务状态**：任务可能未完全完成。\n\n\
    **发生了什么**：代理在执行 {iterations} 次迭代后停止，以防止无限循环。\n\n\
    **下一步操作**：\n\
    1. 检查任务是否已完成\n\
    2. 如未完成，您可以：\n\
       - 提供更具体的指令继续执行\n\
       - 将任务拆分为更小的子任务\n\
       - 使用 '/resume' 从当前状态继续\n\n\
    **限制原因**：防止失控操作和资源耗尽。\n\
    **可调整**：未来版本将支持自定义迭代次数限制。";
