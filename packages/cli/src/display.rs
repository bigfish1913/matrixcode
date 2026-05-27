//! Terminal display helpers for MatrixCode CLI

use termimad::MadSkin;
use termimad::gray;

/// Create a custom skin for MatrixCode terminal output
pub fn create_matrixcode_skin() -> MadSkin {
    let mut skin = MadSkin::default();

    // Headers: cyan color with bold
    skin.headers[0]
        .compound_style
        .set_fg(termimad::crossterm::style::Color::Cyan);
    skin.headers[0].add_attr(termimad::crossterm::style::Attribute::Bold);

    skin.headers[1]
        .compound_style
        .set_fg(termimad::crossterm::style::Color::DarkCyan);
    skin.headers[1].add_attr(termimad::crossterm::style::Attribute::Bold);

    skin.headers[2]
        .compound_style
        .set_fg(termimad::crossterm::style::Color::Yellow);

    // Bold text: white with bold
    skin.bold.set_fg(termimad::crossterm::style::Color::White);
    skin.bold
        .add_attr(termimad::crossterm::style::Attribute::Bold);

    // Inline code: yellow text, gray background
    skin.inline_code
        .set_fg(termimad::crossterm::style::Color::Yellow);
    skin.inline_code.set_bg(gray(20));

    // Code blocks: gray text, dark background
    skin.code_block.set_fg(gray(15));
    skin.code_block.set_bg(gray(5));

    // Italic: gray
    skin.italic.set_fg(gray(12));

    skin
}

/// Print markdown text to terminal with styling
pub fn print_markdown(text: &str) {
    let skin = create_matrixcode_skin();
    skin.print_text(text);
}

/// Print response/result text with border, skipping thinking prefixes
pub fn print_response_border(title: &str, text: &str) {
    // Skip if this was the thinking message we already showed
    if !text.contains("<thinking>") && !text.starts_with("Let me") && !text.starts_with("I need to")
    {
        println!();
        println!("📝 {}:", title);
        print_separator();
        print_markdown(text);
        print_separator();
    }
}

/// Print thinking output with border
pub fn print_thinking_border(text: &str) {
    println!();
    println!("💭 Thinking:");
    print_separator();
    let clean_text = text.replace("<thinking>", "").replace("</thinking>", "");
    println!("{}", clean_text.trim());
    print_separator();
}

/// Print a separator line
pub fn print_separator() {
    println!("─{}", "─".repeat(40));
}
