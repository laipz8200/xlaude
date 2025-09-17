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

/// Drain any remaining piped input to prevent it from being passed to child processes
/// Note: We don't actually drain because tools like 'yes' provide infinite input.
/// Instead, we'll just ensure stdin is not inherited by child processes.
pub fn drain_stdin() -> Result<()> {
    // We used to try to drain all input here, but that causes problems with
    // tools like 'yes' that provide infinite input. The actual solution is
    // to not inherit stdin in child processes (using Stdio::null()).
    Ok(())
}

/// Read a single-choice input with support for piped input and defaults.
/// Returns the canonical key from `valid_keys` that matches the user's selection.
pub fn smart_choice(prompt: &str, valid_keys: &[&str], default_key: &str) -> Result<String> {
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
    let default_normalized = default_key.to_lowercase();

    if let Some(input) = read_piped_line()? {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default_key.to_string());
        }

        let normalized = trimmed.to_lowercase();
        if let Some(index) = normalized_keys.iter().position(|key| key == &normalized) {
            return Ok(valid_keys[index].to_string());
        }

        anyhow::bail!("Invalid selection: {}", trimmed);
    }

    if std::env::var("XLAUDE_NON_INTERACTIVE").is_ok() {
        return Ok(valid_keys
            .iter()
            .find(|key| key.eq_ignore_ascii_case(default_key))
            .unwrap()
            .to_string());
    }

    let default_result = valid_keys
        .iter()
        .find(|key| key.eq_ignore_ascii_case(default_key))
        .unwrap()
        .to_string();

    struct RawModeGuard;
    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
        }
    }

    let raw_mode_enabled = enable_raw_mode().is_ok();
    let _guard = raw_mode_enabled.then_some(RawModeGuard);

    if !raw_mode_enabled {
        return read_line_choice(prompt, &normalized_keys, valid_keys, &default_normalized);
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
                            let selection = valid_keys[index].to_string();
                            println!("{}", selection);
                            io::stdout().flush()?;
                            return Ok(selection);
                        }
                    }
                    KeyCode::Enter => {
                        println!();
                        io::stdout().flush()?;
                        return Ok(default_result);
                    }
                    KeyCode::Esc => {
                        println!();
                        io::stdout().flush()?;
                        return Ok(default_result);
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

fn read_line_choice(
    prompt: &str,
    normalized_keys: &[String],
    valid_keys: &[&str],
    default_normalized: &str,
) -> Result<String> {
    loop {
        if !prompt.is_empty() {
            print!("{}", prompt);
            io::stdout().flush()?;
        }

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();

        let selected = if trimmed.is_empty() {
            default_normalized.to_string()
        } else {
            trimmed.to_lowercase()
        };

        if let Some(index) = normalized_keys.iter().position(|key| key == &selected) {
            return Ok(valid_keys[index].to_string());
        }

        println!(
            "Invalid selection. Please choose from: {}",
            valid_keys.join(", ")
        );
    }
}
