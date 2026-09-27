use std::env;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use ferricula_server::config::RuntimeConfig;
use ferricula_cognition::casting::cast_hexagram;
use ferricula_cognition::entropy::EntropySource;
use ferricula_server::{api, inspect_data_dir};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            println!("usage: ferricula-server inspect <data-dir> [--json]");
            println!("       ferricula-server serve --config <runtime.toml>");
            println!("       ferricula-server init <memory-dir> --agent-id <id> --name <name> [--role <role>]");
        }
        Some("serve") => {
            let Some(flag) = args.next() else {
                bail!("usage: ferricula-server serve --config <runtime.toml>");
            };
            if flag != "--config" {
                bail!("expected --config, got {flag:?}");
            }
            let Some(path) = args.next() else {
                bail!("usage: ferricula-server serve --config <runtime.toml>");
            };
            let config = RuntimeConfig::load(path)?;
            let inspection = inspect_data_dir(&config.memory_dir)?;
            let bind = config.bind;
            let runtime = ferricula_server::runtime::AgentRuntime::open(config, inspection)?;
            tokio::spawn(runtime.clone().run_worker());
            tokio::spawn(runtime.clone().run_scheduler());
            tokio::spawn(runtime.clone().run_life());
            let listener = tokio::net::TcpListener::bind(bind)
                .await
                .with_context(|| format!("failed to bind {bind}"))?;
            eprintln!("Ferricula agent runtime listening on http://{bind}");
            axum::serve(listener, api::router(runtime)).await?;
        }
        Some("inspect") => {
            let Some(data_dir) = args.next() else {
                bail!("usage: ferricula-server inspect <data-dir> [--json]");
            };
            let json = args.next().as_deref() == Some("--json");
            let inspection = inspect_data_dir(data_dir)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                println!("{} ({})", inspection.name, inspection.agent_id);
                println!("rows={} memories={}", inspection.rows, inspection.memories);
                println!(
                    "active={} forgiven={} archived={} keystones={}",
                    inspection.active,
                    inspection.forgiven,
                    inspection.archived,
                    inspection.keystones
                );
                println!(
                    "graph={} nodes/{} edges prime_tree={} terms/{} nodes/{} members",
                    inspection.graph_nodes,
                    inspection.graph_edges,
                    inspection.prime_tree_terms,
                    inspection.prime_tree_nodes,
                    inspection.prime_tree_members
                );
            }
        }
        Some("init") => {
            let Some(dir) = args.next() else {
                bail!("{INIT_USAGE}");
            };
            let (mut agent_id, mut name, mut role) = (None, None, None);
            while let Some(flag) = args.next() {
                let value = args.next().with_context(|| format!("{flag} needs a value"))?;
                match flag.as_str() {
                    "--agent-id" => agent_id = Some(value),
                    "--name" => name = Some(value),
                    "--role" => role = Some(value),
                    other => bail!("unknown flag {other:?}\n{INIT_USAGE}"),
                }
            }
            let (Some(agent_id), Some(name)) = (agent_id, name) else {
                bail!("{INIT_USAGE}");
            };
            init_memory_dir(Path::new(&dir), &agent_id, &name, role.as_deref())?;
        }
        Some(command) => bail!("unknown command {command:?}"),
    }
    Ok(())
}

const INIT_USAGE: &str =
    "usage: ferricula-server init <memory-dir> --agent-id <id> --name <name> [--role <role>]";

/// Create a new agent: `identity.json` (with a birth hexagram cast from the
/// entropy source, which is recorded) and a starter `agent.toml` persona.
/// The memory store itself starts empty; experience accumulates in the
/// runtime's state dir. Refuses to overwrite an existing identity.
fn init_memory_dir(dir: &Path, agent_id: &str, name: &str, role: Option<&str>) -> Result<()> {
    if agent_id.trim().is_empty()
        || !agent_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("agent id must be non-empty ASCII letters, digits, '-' or '_'");
    }
    let identity_path = dir.join("identity.json");
    if identity_path.exists() {
        bail!("{} already exists; refusing to overwrite an agent's identity", identity_path.display());
    }
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;

    let entropy = EntropySource::from_env();
    let mut bytes = Vec::with_capacity(8);
    let first = entropy.draw();
    bytes.extend_from_slice(&first.value.to_le_bytes());
    let hexagram = cast_hexagram(&bytes);
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let identity = serde_json::json!({
        "agent_id": agent_id,
        "name": name,
        "created_at": created_at,
        "hexagram": hexagram,
        "birth_entropy_source": first.source.as_str(),
    });
    fs::write(&identity_path, serde_json::to_vec_pretty(&identity)?)?;

    let persona_path = dir.join("agent.toml");
    if !persona_path.exists() {
        let role = role.unwrap_or("a new Ferricula agent, still discovering what it cares about");
        let persona = format!(
            "# Persona for {name}. Edit freely; the runtime reads name, role and voice.\n\
             name = {name:?}\n\
             role = {role:?}\n\n\
             [voice]\n\
             tone = \"plain, curious, honest about what it does not know\"\n"
        );
        fs::write(&persona_path, persona)?;
    }
    println!(
        "created {name} ({agent_id}) in {}; birth hexagram {} {} (entropy: {})",
        dir.display(),
        hexagram.number,
        hexagram.name,
        first.source.as_str()
    );
    println!("set expected_agent_id = {agent_id:?} and [models.identity] agent_id/name in your runtime config");
    Ok(())
}
