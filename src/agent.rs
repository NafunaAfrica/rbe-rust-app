//! Site agent scaffold, powered by [`rig`](https://docs.rig.rs).
//!
//! This is intentionally a thin skeleton. The intent (to be built out in later
//! updates) is an agent that manages aspects of the store on the admin's
//! behalf: triggering Printify syncs, editing product/design metadata in
//! SurrealDB, drafting journal posts, and answering questions about catalog and
//! sync state. Those capabilities will be added as `rig` **tools** (functions
//! the model can call) — see the `TODO` below.
//!
//! For now it constructs a Claude-backed agent with a preamble describing its
//! role, so the wiring compiles and can be exercised end-to-end once
//! `ANTHROPIC_API_KEY` is set.

#![allow(dead_code)] // scaffold: fields/methods used once the agent is built out

use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers::anthropic;

use crate::config::Config;

const PREAMBLE: &str = "\
You are the RBE (Rich B Energy) site agent. RBE is a print-on-demand slogan-tee \
brand. You help the store owner manage the site: products and designs, Printify \
sync, and journal content. Be concise, direct, and on-brand: loud, unapologetic, \
never corporate. When asked to perform an action you don't yet have a tool for, \
say so plainly rather than pretending. Never invent product data.";

/// Handle to the site agent. Cheap to clone/construct; the underlying `rig`
/// agent is built per request.
#[derive(Clone)]
pub struct SiteAgent {
    model: String,
    enabled: bool,
}

impl SiteAgent {
    pub fn new(cfg: &Config) -> Self {
        SiteAgent {
            model: cfg.agent_model.clone(),
            enabled: cfg.anthropic_api_key.is_some(),
        }
    }

    /// Whether an API key is configured. When false, `handle` returns an error
    /// instead of calling the model.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Send a single prompt to the agent and return its text reply.
    ///
    /// TODO(agent): register tools here via `.tool(...)` so the agent can
    /// actually act — e.g. `SyncProductTool`, `ListProductsTool`,
    /// `EditProductTool`, `DraftJournalPostTool` — each wrapping the existing
    /// services/DB. Then switch to `prompt(...).multi_turn(...)` so it can chain
    /// tool calls.
    pub async fn handle(&self, input: &str) -> anyhow::Result<String> {
        if !self.enabled {
            anyhow::bail!("agent is disabled: set ANTHROPIC_API_KEY to enable it");
        }
        // `from_env` reads ANTHROPIC_API_KEY (loaded from .env at startup).
        let client = anthropic::Client::from_env()
            .map_err(|e| anyhow::anyhow!("failed to init Anthropic client: {e:?}"))?;

        let agent = client.agent(self.model.as_str()).preamble(PREAMBLE).build();

        let reply = agent
            .prompt(input)
            .await
            .map_err(|e| anyhow::anyhow!("agent prompt failed: {e:?}"))?;
        Ok(reply)
    }
}
