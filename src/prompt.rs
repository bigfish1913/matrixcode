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
- 如果需求存在歧义，且该歧义会阻碍安全推进，先提出简洁的澄清问题。
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
- 当下一步明显且风险较低时，无需额外确认即可继续。"#;

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

#[cfg(test)]
mod tests {
    use super::{
        PromptContext, PromptProfile, PromptSection, SystemPromptBuilder,
        SECTION_AVAILABLE_SKILLS, build_static_system_prompt,
    };

    #[test]
    fn prompt_profile_parses_known_values() {
        assert_eq!("default".parse::<PromptProfile>().unwrap(), PromptProfile::Default);
        assert_eq!("safe".parse::<PromptProfile>().unwrap(), PromptProfile::Safe);
        assert_eq!("fast".parse::<PromptProfile>().unwrap(), PromptProfile::Fast);
        assert_eq!("review".parse::<PromptProfile>().unwrap(), PromptProfile::Review);
    }

    #[test]
    fn prompt_profile_rejects_unknown_value() {
        let err = "unknown".parse::<PromptProfile>().unwrap_err();
        assert!(err.contains("unknown prompt profile"));
    }

    #[test]
    fn prompt_profile_default_name_is_stable() {
        assert_eq!(PromptProfile::default().as_str(), "default");
    }

    #[test]
    fn safe_profile_omits_execution_policy() {
        let prompt = build_static_system_prompt(PromptProfile::Safe);
        assert!(!prompt.contains("执行策略："));
        assert!(prompt.contains("行为约束："));
        assert!(prompt.contains("编辑规则："));
    }

    #[test]
    fn fast_profile_omits_behavior_and_editing_rules() {
        let prompt = build_static_system_prompt(PromptProfile::Fast);
        assert!(prompt.contains("执行策略："));
        assert!(!prompt.contains("行为约束："));
        assert!(!prompt.contains("编辑规则："));
    }

    #[test]
    fn review_profile_omits_editing_and_execution_rules() {
        let prompt = build_static_system_prompt(PromptProfile::Review);
        assert!(prompt.contains("行为约束："));
        assert!(!prompt.contains("编辑规则："));
        assert!(!prompt.contains("执行策略："));
    }

    #[test]
    fn prompt_section_renders_with_named_header() {
        let section = PromptSection::new("TASK CONTEXT", "- current task: review").unwrap();
        assert_eq!(section.render(), "[TASK CONTEXT]\n- current task: review");
    }

    #[test]
    fn prompt_section_skips_blank_title_or_body() {
        assert!(PromptSection::new("", "body").is_none());
        assert!(PromptSection::new("TITLE", "   ").is_none());
    }

    #[test]
    fn prompt_context_renders_multiple_sections_in_order() {
        let context = PromptContext::new()
            .with_section("PROJECT CONTEXT", "- language: Rust")
            .with_section("TASK CONTEXT", "- mode: explain");
        assert_eq!(
            context.render_sections(),
            vec![
                "[PROJECT CONTEXT]\n- language: Rust".to_string(),
                "[TASK CONTEXT]\n- mode: explain".to_string()
            ]
        );
    }

    #[test]
    fn builder_appends_named_sections_after_static_prompt() {
        let prompt = SystemPromptBuilder::new(PromptProfile::Default)
            .with_section("DYNAMIC", "- foo")
            .build();
        assert!(prompt.contains("完成要求："));
        assert!(prompt.ends_with("[DYNAMIC]\n- foo"));
    }

    #[test]
    fn builder_accepts_structured_context() {
        let context = PromptContext::new().with_available_skills("- demo: does stuff");
        let prompt = SystemPromptBuilder::new(PromptProfile::Default)
            .with_context(context)
            .build();
        assert!(prompt.contains(&format!("[{}]\n- demo: does stuff", SECTION_AVAILABLE_SKILLS)));
    }

    #[test]
    fn builder_renders_named_skills_section_after_static_prompt() {
        let prompt = SystemPromptBuilder::new(PromptProfile::Default)
            .with_available_skills(
                "Use the `skill` tool with the skill's name to load its full instructions:\n- demo: does stuff",
            )
            .build();
        assert!(prompt.contains("完成要求："));
        assert!(prompt.contains(&format!("[{}]", SECTION_AVAILABLE_SKILLS)));
        assert!(prompt.contains("- demo: does stuff"));
    }
}
