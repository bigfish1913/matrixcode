use clap::Parser;

#[derive(Parser)]
#[command(name = "calculator")]
#[command(about = "Simple calculator CLI")]
struct Cli {
    /// First number
    #[arg(short, long)]
    a: f64,
    
    /// Second number
    #[arg(short, long)]
    b: f64,
    
    /// Operation: add, sub, mul, div
    #[arg(short, long)]
    op: String,
}

fn main() {
    let cli = Cli::parse();
    
    let result = match cli.op.as_str() {
        "add" => cli.a + cli.b,
        "sub" => cli.a - cli.b,
        "mul" => cli.a * cli.b,
        "div" => {
            if cli.b == 0.0 {
                eprintln!("Error: Division by zero");
                return;
            }
            cli.a / cli.b
        },
        _ => {
            eprintln!("Error: Unknown operation '{}'", cli.op);
            return;
        }
    };
    
    println!("Result: {} {} {} = {}", cli.a, cli.op, cli.b, result);
}
