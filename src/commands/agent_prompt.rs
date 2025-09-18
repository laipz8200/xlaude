use anyhow::Result;

use crate::input::smart_choice;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentSelection {
    Codex,
    DefaultAgent,
    Skip,
}

impl AgentSelection {
    fn as_key(self) -> &'static str {
        match self {
            AgentSelection::Codex => "1",
            AgentSelection::DefaultAgent => "2",
            AgentSelection::Skip => "n",
        }
    }
}

pub fn prompt_agent_selection(
    prompt: &str,
    agent_display: &str,
    default_choice: AgentSelection,
) -> Result<AgentSelection> {
    if !prompt.is_empty() {
        println!("{}", prompt);
    }
    println!("1. Open with `codex`.");
    println!("2. Open with `{}`.", agent_display);
    println!("n. Don't open.");

    let choice = smart_choice("> ", &["1", "2", "n"], default_choice.as_key())?;
    Ok(match choice.as_str() {
        "1" => AgentSelection::Codex,
        "2" => AgentSelection::DefaultAgent,
        _ => AgentSelection::Skip,
    })
}
