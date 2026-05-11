use anyhow::Result;
use clap::Parser;
use code_agent::{agent, providers};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

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

    /// One-shot prompt. If omitted, enters interactive REPL mode.
    prompt: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

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

    let mut agent = agent::Agent::with_options(provider, cli.think);

    if !cli.prompt.is_empty() {
        agent.chat_once(&cli.prompt.join(" ")).await?;
        return Ok(());
    }

    run_repl(&mut agent).await
}

async fn run_repl(agent: &mut agent::Agent) -> Result<()> {
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
    }

    if let Some(ref p) = history_path {
        let _ = rl.save_history(p);
    }

    Ok(())
}

fn dirs_history_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut p = std::path::PathBuf::from(home);
    p.push(".code-agent_history");
    Some(p)
}
