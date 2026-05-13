use anyhow::{Context, Result};
use clap::Parser;
use code_agent::{
    agent,
    prompt,
    providers::{self, Message},
    skills,
};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "code-agent", about = "A simple code agent with tool use")]
struct Cli {
    #[arg(short, long, env = "PROVIDER", default_value = "anthropic")]
    provider: String,

    #[arg(short, long, env = "MODEL_NAME")]
    model: Option<String>,

    #[arg(long, env = "API_KEY")]
    api_key: Option<String>,

    #[arg(long, env = "BASE_URL")]
    base_url: Option<String>,

    /// Enable Anthropic extended thinking (default on). Pass --think false to disable.
    #[arg(long, env = "THINK", default_value_t = true, action = clap::ArgAction::Set)]
    think: bool,

    /// Render assistant output as Markdown in the terminal (default on).
    /// Pass --markdown false to disable, or set NO_COLOR / run in a non-TTY
    /// to auto-disable.
    #[arg(long, env = "MARKDOWN", default_value_t = true, action = clap::ArgAction::Set)]
    markdown: bool,

    /// Resume the previous session from disk instead of starting fresh.
    /// Without this flag the saved session is overwritten by the new one.
    #[arg(short, long, env = "RESUME", default_value_t = false)]
    resume: bool,

    /// Extra directory to scan for skills. May be passed multiple times.
    /// The defaults `./skills` and `~/.code-agent/skills` are always
    /// scanned first unless `--no-default-skills` is set.
    #[arg(long = "skills-dir", env = "SKILLS_DIR", value_delimiter = ':')]
    skills_dir: Vec<PathBuf>,

    /// Skip the default skills roots (`./skills`, `~/.code-agent/skills`).
    #[arg(long, default_value_t = false)]
    no_default_skills: bool,

    /// Prompt profile: default, safe, fast, review.
    #[arg(long, env = "PROMPT_PROFILE", default_value = "default")]
    profile: String,

    /// Enable server-side web search tool (default on for Anthropic provider).
    /// The model can perform web searches directly via the API provider.
    /// Pass --no-web-search to disable.
    #[arg(long, env = "NO_WEB_SEARCH", default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_web_search: bool,

    /// Maximum number of web searches per turn when --web-search is enabled.
    #[arg(long, env = "WEB_SEARCH_MAX_USES", default_value = "5")]
    web_search_max_uses: u32,

    /// One-shot prompt. If omitted, enters interactive REPL mode.
    prompt: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();

    let cli = Cli::parse();

    let api_key = cli.api_key.unwrap_or_else(|| match cli.provider.as_str() {
        "openai" => std::env::var("OPENAI_API_KEY").expect("API_KEY or OPENAI_API_KEY required"),
        _ => std::env::var("ANTHROPIC_API_KEY").expect("API_KEY or ANTHROPIC_API_KEY required"),
    });

    let model = cli.model.unwrap_or_else(|| match cli.provider.as_str() {
        "openai" => "gpt-4o".to_string(),
        _ => "claude-sonnet-4-20250514".to_string(),
    });

    let base_url = cli.base_url.unwrap_or_else(|| match cli.provider.as_str() {
        "openai" => "https://api.openai.com/v1".to_string(),
        _ => "https://api.anthropic.com".to_string(),
    });

    let provider: Box<dyn providers::Provider> = match cli.provider.as_str() {
        "openai" => Box::new(providers::openai::OpenAIProvider::new(
            api_key, model, base_url,
        )),
        "anthropic" => Box::new(providers::anthropic::AnthropicProvider::new(
            api_key, model, base_url,
        )),
        other => anyhow::bail!("Unknown provider: {other}. Use 'openai' or 'anthropic'"),
    };

    let profile = cli
        .profile
        .parse::<prompt::PromptProfile>()
        .map_err(anyhow::Error::msg)?;

    let mut agent = agent::Agent::with_profile_and_skills(
        provider,
        cli.think,
        cli.markdown,
        profile,
        load_skills(&cli.skills_dir, cli.no_default_skills),
    );

    // Enable server-side web search by default for Anthropic provider
    if cli.provider == "anthropic" && !cli.no_web_search {
        agent = agent.with_web_search(Some(cli.web_search_max_uses));
        println!("[server web search enabled, max {} uses per turn]", cli.web_search_max_uses);
    }

    let session_path = dirs_session_path();
    if cli.resume {
        match session_path.as_ref().and_then(|p| load_session(p).transpose()) {
            Some(Ok(messages)) => {
                let n = messages.len();
                agent.set_messages(messages);
                println!("[resumed session with {n} message(s)]");
            }
            Some(Err(e)) => {
                eprintln!("[warn] could not resume session: {e}. Starting fresh.");
            }
            None => {
                println!("[no prior session found, starting fresh]");
            }
        }
    } else if let Some(ref p) = session_path {
        // New session: clear any prior saved state so --resume later picks
        // up only what happens in this run.
        let _ = std::fs::remove_file(p);
    }

    if !cli.prompt.is_empty() {
        agent.chat_once(&cli.prompt.join(" ")).await?;
        if let Some(ref p) = session_path {
            if let Err(e) = save_session(p, agent.messages()) {
                eprintln!("[warn] could not save session: {e}");
            }
        }
        return Ok(());
    }

    run_repl(&mut agent, session_path.as_deref()).await
}

async fn run_repl(agent: &mut agent::Agent, session_path: Option<&Path>) -> Result<()> {
    println!("code-agent — type your message and press Enter. Ctrl+D or /exit to quit.");

    let mut rl = DefaultEditor::new()?;
    let history_path = dirs_history_path();
    if let Some(ref p) = history_path {
        let _ = rl.load_history(p);
    }

    loop {
        let line = match rl.readline("\n> ") {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C at the prompt: cancel current input, stay in REPL.
                continue;
            }
            Err(ReadlineError::Eof) => break, // Ctrl+D
            Err(e) => {
                eprintln!("input error: {e}");
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if matches!(trimmed, "/exit" | "/quit" | ":q") {
            break;
        }

        let _ = rl.add_history_entry(trimmed);

        if let Err(e) = agent.chat_once(trimmed).await {
            eprintln!("\n[error] {e}");
        }

        if let Some(p) = session_path {
            if let Err(e) = save_session(p, agent.messages()) {
                eprintln!("[warn] could not save session: {e}");
            }
        }
    }

    if let Some(ref p) = history_path {
        let _ = rl.save_history(p);
    }

    Ok(())
}

/// Resolve the list of directories to scan for skills and load them.
/// Order matters: earlier roots win on duplicate names, so user-provided
/// `--skills-dir` entries take precedence over the defaults.
fn load_skills(extra: &[PathBuf], skip_defaults: bool) -> Vec<skills::Skill> {
    let mut roots: Vec<PathBuf> = Vec::new();
    roots.extend(extra.iter().cloned());
    if !skip_defaults {
        roots.push(PathBuf::from("skills"));
        if let Some(home) = std::env::var_os("HOME") {
            let mut p = PathBuf::from(home);
            p.push(".code-agent");
            p.push("skills");
            roots.push(p);
        }
    }
    let found = skills::discover_skills(&roots);
    if !found.is_empty() {
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        println!("[loaded {} skill(s): {}]", found.len(), names.join(", "));
    }
    found
}

fn dirs_history_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut p = PathBuf::from(home);
    p.push(".code-agent_history");
    Some(p)
}

fn dirs_session_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut p = PathBuf::from(home);
    p.push(".code-agent_session.json");
    Some(p)
}

/// Load a previously saved session. Returns `Ok(None)` when no file exists,
/// so "first-time run with --resume" is not treated as an error.
fn load_session(path: &Path) -> Result<Option<Vec<Message>>> {
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("reading session file {}", path.display()))?;
    if data.trim().is_empty() {
        return Ok(None);
    }
    let messages: Vec<Message> = serde_json::from_str(&data)
        .with_context(|| format!("parsing session file {}", path.display()))?;
    Ok(Some(messages))
}

/// Persist the conversation atomically (write to a temp file in the same
/// directory, then rename) so a crash mid-write can't leave a truncated JSON.
fn save_session(path: &Path, messages: &[Message]) -> Result<()> {
    let json = serde_json::to_string(messages).context("serializing session")?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)
        .with_context(|| format!("writing session tmp file {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming tmp session file to {}", path.display()))?;
    Ok(())
}
