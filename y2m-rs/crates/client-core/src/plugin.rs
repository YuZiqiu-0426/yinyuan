use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use y2m_common::{EventPacket, EventType};

use crate::{connection::ClientConnection, session::ClientIdentity};

#[derive(Clone)]
pub struct PluginContext {
    pub identity: ClientIdentity,
    pub connection: ClientConnection,
}

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self) -> &'static [EventType];
    async fn on_event(
        &self,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin>>,
    routes: HashMap<EventType, Vec<Arc<dyn Plugin>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plugin: Arc<dyn Plugin>) {
        for event_type in plugin.supports() {
            self.routes
                .entry(*event_type)
                .or_default()
                .push(plugin.clone());
        }
        self.plugins.push(plugin);
    }

    pub async fn dispatch(
        &self,
        ctx: &PluginContext,
        packet: &EventPacket,
    ) -> anyhow::Result<()> {
        if let Some(plugins) = self.routes.get(&packet.payload.event_type) {
            for plugin in plugins {
                plugin.on_event(ctx, packet).await?;
            }
        }

        Ok(())
    }
}
