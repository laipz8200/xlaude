use anyhow::Result;
use atty::Stream;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use dialoguer::{Confirm, Select};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::Mutex;

/// Check if stdin is piped (not a terminal)
pub fn is_piped_input() -> bool {
    !atty::is(Stream::Stdin)
}

/// Piped input reader that supports reading multiple lines
pub struct PipedInputReader {
    reader: BufReader<io::Stdin>,
    buffer: Vec<String>,
}

impl PipedInputReader {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
            buffer: Vec::new(),
        }
    }

    /// Read the next line of input
    pub fn read_line(&mut self) -> Result<Option<String>> {
        // Use buffered input first if available
        if !self.buffer.is_empty() {
            return Ok(Some(self.buffer.remove(0)));
        }

        let mut line = String::new();
        match self.reader.read_line(&mut line)? {
            0 => Ok(None), // EOF
            _ => Ok(Some(line.trim().to_string())),
        }
    }
}

/// Global piped input reader (singleton)
static PIPED_INPUT: std::sync::LazyLock<Mutex<Option<PipedInputReader>>> =
    std::sync::LazyLock::new(|| {
        if is_piped_input() {
            Mutex::new(Some(PipedInputReader::new()))
        } else {
            Mutex::new(None)
        }
    });

/// Read a single line from piped input
pub fn read_piped_line() -> Result<Option<String>> {
    let mut reader = PIPED_INPUT.lock().unwrap();
    match reader.as_mut() {
        Some(r) => r.read_line(),
        None => Ok(None),
    }
}

/// Smart confirmation that supports piped input (yes/no)
pub fn smart_confirm(prompt: &str, default: bool) -> Result<bool> {
    // 1. Check for force-yes environment variable
    if std::env::var("XLAUDE_YES").is_ok() {
        return Ok(true);
    }

    // 2. Check for piped input
    if let Some(input) = read_piped_line()? {
        let input = input.to_lowercase();
        return Ok(input == "y" || input == "yes");
    }

    // 3. Non-interactive mode uses default value
    if std::env::var("XLAUDE_NON_INTERACTIVE").is_ok() {
        return Ok(default);
    }

    // 4. Interactive confirmation
    Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()
        .map_err(Into::into)
}

/// Smart selection that supports piped input
pub fn smart_select<T>(
    prompt: &str,
    items: &[T],
    display_fn: impl Fn(&T) -> String,
) -> Result<Option<usize>>
where
    T: Clone,
{
    // 1. Check for piped input
    if let Some(input) = read_piped_line()? {
        // Try to parse as index
        if let Ok(index) = input.parse::<usize>()
            && index < items.len()
        {
            return Ok(Some(index));
        }

        // Try to match display text
        for (i, item) in items.iter().enumerate() {
            if display_fn(item) == input {
                return Ok(Some(i));
            }
        }

        anyhow::bail!("Invalid selection: {}", input);
    }

    // 2. Non-interactive mode returns None
    if std::env::var("XLAUDE_NON_INTERACTIVE").is_ok() {
        return Ok(None);
    }

    // 3. Interactive selection
    let display_items: Vec<String> = items.iter().map(display_fn).collect();
    let selection = Select::new()
        .with_prompt(prompt)
        .items(&display_items)
        .interact()?;

    Ok(Some(selection))
}

/// Get command argument with pipe input support
/// Priority: CLI argument > piped input > None
pub fn get_command_arg(arg: Option<String>) -> Result<Option<String>> {
    // 1. CLI argument takes priority
    if arg.is_some() {
        return Ok(arg);
    }

    // 2. Check piped input (skip yes/no confirmation words)
    // Only try to read once to avoid getting stuck with infinite streams like 'yes'
    if let Some(input) = read_piped_line()? {
        let lower = input.to_lowercase();
        // Skip confirmation words that might be in the pipe
        // These are likely from tools like 'yes' and not meant as actual input
        if lower != "y" && lower != "yes" && lower != "n" && lower != "no" {
            return Ok(Some(input));
        }
        // If it's a confirmation word, return None to let the command use defaults
    }

    Ok(None)
}

/// Read a single-choice input with support for piped input and defaults.
/// Returns the canonical key from `valid_keys` that matches the user's selection.
#[allow(dead_code)]
pub fn smart_choice(prompt: &str, valid_keys: &[&str], default_key: &str) -> Result<String> {
    smart_choice_with_formatter(prompt, valid_keys, default_key, |key| key.to_string())
}

pub fn smart_choice_with_formatter<F>(
    prompt: &str,
    valid_keys: &[&str],
    default_key: &str,
    format_selected: F,
) -> Result<String>
where
    F: Fn(&str) -> String,
{
    if !valid_keys
        .iter()
        .any(|key| key.eq_ignore_ascii_case(default_key))
    {
        anyhow::bail!(
            "Default choice '{}' is not present in the list of valid options",
            default_key
        );
    }

    let normalized_keys: Vec<String> = valid_keys.iter().map(|key| key.to_lowercase()).collect();
    let default_index = normalized_keys
        .iter()
        .position(|key| key.eq_ignore_ascii_case(&default_key.to_lowercase()))
        .unwrap();
    let default_key_canonical = valid_keys[default_index];

    if let Some(input) = read_piped_line()? {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Ok(default_key_canonical.to_string());
        }

        let normalized = trimmed.to_lowercase();

        let alias_key = match normalized.as_str() {
            "y" | "yes" => Some(default_key.to_lowercase()),
            "n" | "no" => Some("n".to_string()),
            _ => None,
        }
        .and_then(|alias| {
            normalized_keys
                .iter()
                .position(|key| key == &alias)
                .map(|index| valid_keys[index].to_string())
        });

        if let Some(mapped) = alias_key {
            return Ok(mapped);
        }

        if let Some(index) = normalized_keys.iter().position(|key| key == &normalized) {
            return Ok(valid_keys[index].to_string());
        }

        anyhow::bail!("Invalid selection: {}", trimmed);
    }

    if std::env::var("XLAUDE_NON_INTERACTIVE").is_ok() {
        return Ok(default_key_canonical.to_string());
    }

    struct RawModeGuard;
    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
        }
    }

    let raw_mode_enabled = enable_raw_mode().is_ok();
    let _guard = raw_mode_enabled.then_some(RawModeGuard);

    if !raw_mode_enabled {
        return read_line_choice(
            prompt,
            &normalized_keys,
            valid_keys,
            default_index,
            &format_selected,
        );
    }

    if !prompt.is_empty() {
        print!("{}", prompt);
        io::stdout().flush()?;
    }

    loop {
        match event::read()? {
            Event::Key(key_event)
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
            {
                match key_event.code {
                    KeyCode::Char(c) => {
                        if key_event.modifiers.contains(KeyModifiers::CONTROL)
                            && (c == 'c' || c == 'C')
                        {
                            println!();
                            return Err(anyhow::anyhow!("Operation cancelled by Ctrl+C"));
                        }

                        let normalized = c.to_ascii_lowercase().to_string();
                        if let Some(index) =
                            normalized_keys.iter().position(|key| key == &normalized)
                        {
                            let selection = valid_keys[index];
                            let rendered = format_selected(selection);
                            if prompt.is_empty() {
                                println!("{}", rendered);
                            } else {
                                print!("\r{}{}\n", prompt, rendered);
                            }
                            io::stdout().flush()?;
                            return Ok(selection.to_string());
                        }
                    }
                    KeyCode::Enter | KeyCode::Esc => {
                        let selection = valid_keys[default_index];
                        let rendered = format_selected(selection);
                        if prompt.is_empty() {
                            println!("{}", rendered);
                        } else {
                            print!("\r{}{}\n", prompt, rendered);
                        }
                        io::stdout().flush()?;
                        return Ok(selection.to_string());
                    }
                    _ => {}
                }

                println!(
                    "Invalid selection. Please choose from: {}",
                    valid_keys.join(", ")
                );
                if !prompt.is_empty() {
                    print!("{}", prompt);
                    io::stdout().flush()?;
                }
            }
            _ => {}
        }
    }
}

fn read_line_choice<F>(
    prompt: &str,
    normalized_keys: &[String],
    valid_keys: &[&str],
    default_index: usize,
    format_selected: &F,
) -> Result<String>
where
    F: Fn(&str) -> String,
{
    loop {
        if !prompt.is_empty() {
            print!("{}", prompt);
            io::stdout().flush()?;
        }

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();

        let selected = if trimmed.is_empty() {
            normalized_keys[default_index].clone()
        } else {
            trimmed.to_lowercase()
        };

        if let Some(index) = normalized_keys.iter().position(|key| key == &selected) {
            let selection = valid_keys[index];
            let rendered = format_selected(selection);
            if prompt.is_empty() {
                println!("{}", rendered);
            } else {
                println!("{}{}", prompt, rendered);
            }
            return Ok(selection.to_string());
        }

        println!(
            "Invalid selection. Please choose from: {}",
            valid_keys.join(", ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_choice_uses_default_when_non_interactive() {
        unsafe {
            std::env::set_var("XLAUDE_NON_INTERACTIVE", "1");
        }

        let default_result = smart_choice("> ", &["1", "2"], "2")
            .expect("smart_choice should succeed when non-interactive");
        assert_eq!(default_result, "2");

        let result = smart_choice_with_formatter("> ", &["1", "2"], "2", |_| {
            unreachable!("formatter should not be invoked")
        })
        .expect("smart_choice_with_formatter should succeed");

        assert_eq!(result, "2");

        unsafe {
            std::env::remove_var("XLAUDE_NON_INTERACTIVE");
        }
    }
}
