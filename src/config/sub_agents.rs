use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SubAgentEntry {
    pub name: String,
    pub description: Option<String>,
    pub model: String,
    pub icon: Option<String>,
    #[serde(default)]
    pub system_prompt: String,
    pub tools: Option<String>,
    pub skills: Option<String>,
    #[serde(default)]
    pub default: bool,
}

fn agent_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User scope: ~/.pi/agent/agents/
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    if !home.is_empty() {
        dirs.push(PathBuf::from(&home).join(".pi/agent/agents"));
    }

    // Project scope: .pi/agents/ (relative to current_dir)
    let current_dir = crate::config::current_root::get();
    dirs.push(current_dir.join(".pi/agents"));

    dirs
}

fn parse_agent_file(path: &std::path::Path) -> Option<SubAgentEntry> {
    let content = std::fs::read_to_string(path).ok()?;

    // Find YAML frontmatter between --- markers
    let content = content.trim();
    if !content.starts_with("---") {
        return None;
    }
    let end = content[3..].find("---")?;
    let yaml_str = &content[3..3 + end];

    // Parse YAML frontmatter
    let mut entry: SubAgentEntry = serde_yaml::from_str(yaml_str).ok()?;

    // Extract system_prompt body (everything after the frontmatter and optional blank lines)
    let body = content[3 + end + 3..].trim().to_string();
    if !body.is_empty() {
        entry.system_prompt = body;
    }

    Some(entry)
}

pub fn list() -> Vec<SubAgentEntry> {
    let mut agents = Vec::new();
    for dir in agent_dirs() {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                if let Some(agent) = parse_agent_file(&path) {
                    agents.push(agent);
                }
            }
        }
    }
    agents
}

pub fn get_by_name(name: &str) -> Option<SubAgentEntry> {
    list().into_iter().find(|a| a.name == name)
}

pub fn get_default() -> Option<SubAgentEntry> {
    list().into_iter().find(|a| a.default)
}

pub fn agent_dirs_list() -> Vec<PathBuf> {
    agent_dirs().into_iter().filter(|d| d.exists()).collect()
}

pub fn builtins() -> Vec<&'static str> {
    vec![
        "scout",
        "researcher",
        "planner",
        "worker",
        "reviewer",
        "context-builder",
        "oracle",
        "delegate",
    ]
}
