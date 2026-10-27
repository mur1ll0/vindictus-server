//! Maps and channels (B4.6): a read-only view of the laboratory's single channel, its
//! members, the mission relay, the listening ports and the content version loaded.
use super::admin_session::AdminError;
use crate::{
    domain::admin::Permission,
    infrastructure::{
        admin_characters, admin_store::SessionAccount, content_edits, lab_frontend::Runtime,
        lab_web_shop,
    },
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct WorldView {
    /// Game ports, in the order the client meets them.
    pub ports: Vec<PortView>,
    /// Live frontend connections (login, character list, town).
    pub connections: usize,
    /// The one laboratory channel, when the content configures it.
    pub channel: Option<ChannelView>,
    pub relay: Option<RelayView>,
    pub content: ContentVersionView,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PortView {
    /// frontend, channel, relay or web_shop.
    pub service: &'static str,
    pub address: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ChannelView {
    pub id: i32,
    pub members: Vec<MemberView>,
    /// Characters with a channel ticket whose connection has not identified yet.
    pub entering: usize,
    /// Maps built into the channel and whether each one built.
    pub maps: Vec<MapView>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MemberView {
    pub cid: i64,
    pub name: Option<String>,
    /// Game account of the character.
    pub account: Option<String>,
    pub partition: i64,
    /// Map (bsp) of the partition.
    pub map: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MapView {
    pub name: String,
    pub built: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RelayView {
    pub connected: usize,
    pub waiting: usize,
    /// Missions in progress (P2P groups) and their member counts.
    pub groups: Vec<GroupView>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct GroupView {
    pub id: i32,
    pub members: usize,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ContentVersionView {
    /// Version applied over the contents the game has in memory (0: none).
    pub loaded: Option<i32>,
    /// Newest published version.
    pub published: Option<i32>,
}

pub async fn world(
    pool: &PgPool,
    session: &SessionAccount,
    runtime: Option<&Runtime>,
) -> Result<WorldView, AdminError> {
    if !session.role.is_some_and(|r| r.allows(Permission::Read)) {
        return Err(AdminError::Forbidden);
    }
    let published = content_edits::versions(pool, 1)
        .await?
        .first()
        .map(|v| v.version);
    let Some(runtime) = runtime else {
        return Ok(WorldView {
            ports: Vec::new(),
            connections: 0,
            channel: None,
            relay: None,
            content: ContentVersionView {
                loaded: None,
                published,
            },
        });
    };
    let mut ports = vec![PortView {
        service: "frontend",
        address: runtime.bind().to_string(),
    }];
    let channel = match runtime.channel() {
        Some((bind, channel)) => {
            ports.push(PortView {
                service: "channel",
                address: bind.to_string(),
            });
            let snapshot = channel.snapshot().await;
            let cids: Vec<i64> = snapshot.members.iter().map(|m| m.cid).collect();
            let names: HashMap<i64, (String, String)> = if cids.is_empty() {
                HashMap::new()
            } else {
                admin_characters::names_of(pool, &cids)
                    .await?
                    .into_iter()
                    .map(|(cid, name, account)| (cid, (name, account)))
                    .collect()
            };
            Some(ChannelView {
                id: 1,
                members: snapshot
                    .members
                    .into_iter()
                    .map(|m| {
                        let (name, account) = names.get(&m.cid).cloned().unzip();
                        MemberView {
                            cid: m.cid,
                            name,
                            account,
                            partition: m.partition,
                            map: m.region,
                        }
                    })
                    .collect(),
                entering: snapshot.entering,
                maps: snapshot
                    .maps
                    .into_iter()
                    .map(|(name, built)| MapView { name, built })
                    .collect(),
            })
        }
        None => None,
    };
    let relay = runtime.relay().map(|relay| {
        ports.push(PortView {
            service: "relay",
            address: relay.endpoint().to_string(),
        });
        let s = relay.snapshot();
        RelayView {
            connected: s.connected,
            waiting: s.waiting,
            groups: s
                .groups
                .into_iter()
                .map(|(id, members)| GroupView { id, members })
                .collect(),
        }
    });
    let loaded = runtime.minimum_assets().map(|a| a.content_version);
    if loaded.is_some() {
        ports.push(PortView {
            service: "web_shop",
            address: lab_web_shop::BIND.to_string(),
        });
    }
    Ok(WorldView {
        ports,
        connections: runtime.active_count().unwrap_or(0),
        channel,
        relay,
        content: ContentVersionView { loaded, published },
    })
}
