use anyhow::Result;
use colored::Colorize;

use crate::input::smart_choice_with_formatter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentSelection {
    Codex,
    Claude,
    Skip,
}

impl AgentSelection {
    fn as_key(self) -> &'static str {
        match self {
            AgentSelection::Codex => "1",
            AgentSelection::Claude => "2",
            AgentSelection::Skip => "n",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "1" => Some(AgentSelection::Codex),
            "2" => Some(AgentSelection::Claude),
            "n" | "N" => Some(AgentSelection::Skip),
            _ => None,
        }
    }
}

struct AgentMenuOption {
    selection: AgentSelection,
    title: &'static str,
    command: &'static str,
    description: &'static str,
    confirmation: &'static str,
}

const AGENT_MENU_OPTIONS: [AgentMenuOption; 3] = [
    AgentMenuOption {
        selection: AgentSelection::Codex,
        title: "Open with codex",
        command: "codex",
        description: "Open the worktree in the codex CLI.",
        confirmation: "Launching with `codex`",
    },
    AgentMenuOption {
        selection: AgentSelection::Claude,
        title: "Open with Claude",
        command: "claude --dangerously-skip-permissions",
        description: "Launch using the configured Claude command.",
        confirmation: "Launching with `claude --dangerously-skip-permissions`",
    },
    AgentMenuOption {
        selection: AgentSelection::Skip,
        title: "Skip launch",
        command: "",
        description: "Keep the worktree open without launching an agent.",
        confirmation: "Skipping launch",
    },
];

pub fn prompt_agent_selection(
    prompt: &str,
    default_choice: AgentSelection,
) -> Result<AgentSelection> {
    if !prompt.is_empty() {
        println!("{}", prompt.bold());
        println!();
    }

    for (index, option) in AGENT_MENU_OPTIONS.iter().enumerate() {
        let is_default = option.selection == default_choice;
        let key_label = format!("[{}]", option.selection.as_key().to_uppercase());
        let key_display = if is_default {
            key_label.green().bold()
        } else {
            key_label.cyan()
        };

        let mut title = option.title.to_string();
        if is_default {
            title.push_str(" (default)");
        }

        let title_display = if is_default {
            title.as_str().cyan().bold()
        } else {
            title.as_str().cyan()
        };

        println!("  {} {}", key_display, title_display);

        if !option.command.is_empty() {
            println!(
                "      {} {}",
                "Command:".bright_black(),
                format!("`{}`", option.command).cyan()
            );
        }

        if !option.description.is_empty() {
            println!("      {}", option.description.bright_black());
        }

        if index + 1 != AGENT_MENU_OPTIONS.len() {
            println!();
        }
    }

    println!();
    println!(
        "  Press {}, {} or {}; Enter accepts the default.",
        "[1]".bright_black(),
        "[2]".bright_black(),
        "[N]".bright_black()
    );
    println!();

    let prompt_indicator = format!("{} ", "›".bright_black());
    let valid_keys = ["1", "2", "n"];

    let choice = smart_choice_with_formatter(
        &prompt_indicator,
        &valid_keys,
        default_choice.as_key(),
        |key| {
            let selection = AgentSelection::from_key(key).expect("invalid agent selection key");
            let option = AGENT_MENU_OPTIONS
                .iter()
                .find(|opt| opt.selection == selection)
                .expect("missing agent option");

            match selection {
                AgentSelection::Codex | AgentSelection::Claude => {
                    format!("{} {}", "✔".green(), option.confirmation.cyan())
                }
                AgentSelection::Skip => format!("{} {}", "⏭".yellow(), option.confirmation),
            }
        },
    )?;

    Ok(AgentSelection::from_key(&choice).expect("invalid agent choice"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_selection_key_roundtrip() {
        assert_eq!(
            AgentSelection::from_key(AgentSelection::Codex.as_key()),
            Some(AgentSelection::Codex)
        );
        assert_eq!(
            AgentSelection::from_key(AgentSelection::Claude.as_key()),
            Some(AgentSelection::Claude)
        );
        assert_eq!(
            AgentSelection::from_key(AgentSelection::Skip.as_key()),
            Some(AgentSelection::Skip)
        );
        assert_eq!(AgentSelection::from_key("N"), Some(AgentSelection::Skip));
        assert_eq!(AgentSelection::from_key("invalid"), None);
    }
}
