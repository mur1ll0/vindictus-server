//! Recovered MapFile `.dat` parser, degree-1 partition visibility table, and MMOServer
//! Channel component/observer state machine for the minimum town entry path.
use crate::{
    domain::legacy_user_data::SqlDateTime2,
    protocol::{
        DEFAULT_MAX_BODY,
        character_list::{self, Summary},
        minimum_map::{self, ActionSync, Kind, Message},
    },
};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;
use zeroize::Zeroizing;

pub const MAX_MAP_FILE_BYTES: usize = 256 * 1024;
pub const MAX_PARTITIONS_PER_MAP: usize = 4096;
pub const MAX_LINKS_PER_MAP: usize = 16_384;
const UTILITY_EPOCH_TICKS: i64 = 621_355_968_000_000_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MapPartitionError {
    #[error("map partition file is truncated or malformed")]
    Format,
    #[error("map partition file exceeds bounds")]
    Bounds,
    #[error("channel wire serialization failed")]
    Wire,
}

/// Utility.Extention.ToInt32(this DateTime value): signed seconds since 1970-01-01T00:00:00Z
/// truncated to 32-bit signed integer.
pub fn utility_epoch_seconds(utc: SqlDateTime2) -> i32 {
    ((utc.ticks() - UTILITY_EPOCH_TICKS) / 10_000_000) as i32
}

/// SelectCharacterProcessor: `(RegionCode != 1) ? RegionCode : 0`.
pub const fn login_ok_region_code(region_code: i32) -> i32 {
    if region_code != 1 { region_code } else { 0 }
}

/// MMOChannelService.RecommendChannel with MMOForceRecommend enabled: picks the lowest
/// channel ID in `1..=active_channel_max` with member count `< (int)(capacity * 0.8)`.
pub fn recommend_channel(
    requested_channel_id: i64,
    active_channel_max: i32,
    capacity: i32,
    member_counts: &BTreeMap<i64, usize>,
) -> i64 {
    let max = i64::from(active_channel_max);
    if requested_channel_id >= 1 && requested_channel_id <= max {
        return requested_channel_id;
    }
    let threshold = ((f64::from(capacity) * 0.8).trunc() as i32).max(0) as usize;
    for cid in 1..=max {
        let count = member_counts.get(&cid).copied().unwrap_or(0);
        if count < threshold {
            return cid;
        }
    }
    -1
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapFile {
    bsp: String,
    partitions: Vec<i64>,
    visible_links: Vec<(i64, i64)>,
}

impl MapFile {
    /// Parse a HeroesChannelServer MapFile `.dat` stream (24-byte LE header, i32 count,
    /// `count` i64 LE partition IDs in HashSet insertion order, then (i64, i64) LE link pairs).
    pub fn parse(bsp: impl Into<String>, bytes: &[u8]) -> Result<Self, MapPartitionError> {
        let bsp = bsp.into();
        if bsp.is_empty() || bsp.len() > 256 {
            return Err(MapPartitionError::Format);
        }
        if bytes.len() > MAX_MAP_FILE_BYTES {
            return Err(MapPartitionError::Bounds);
        }
        if bytes.len() < 28 {
            return Err(MapPartitionError::Format);
        }
        let num = i32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let num = usize::try_from(num).map_err(|_| MapPartitionError::Format)?;
        if num > MAX_PARTITIONS_PER_MAP {
            return Err(MapPartitionError::Bounds);
        }
        let links_start = 28usize
            .checked_add(num.checked_mul(8).ok_or(MapPartitionError::Bounds)?)
            .ok_or(MapPartitionError::Bounds)?;
        if bytes.len() < links_start {
            return Err(MapPartitionError::Format);
        }
        let rem = bytes.len() - links_start;
        if !rem.is_multiple_of(16) {
            return Err(MapPartitionError::Format);
        }
        let link_count = rem / 16;
        if link_count > MAX_LINKS_PER_MAP {
            return Err(MapPartitionError::Bounds);
        }
        let mut partitions = Vec::with_capacity(num);
        for chunk in bytes[28..links_start].as_chunks::<8>().0 {
            let pid = i64::from_le_bytes(*chunk);
            if !partitions.contains(&pid) {
                partitions.push(pid);
            }
        }
        let mut visible_links = Vec::with_capacity(link_count);
        for chunk in bytes[links_start..].as_chunks::<16>().0 {
            let lhs = i64::from_le_bytes(chunk[..8].try_into().unwrap());
            let rhs = i64::from_le_bytes(chunk[8..].try_into().unwrap());
            visible_links.push((lhs, rhs));
        }
        Ok(Self {
            bsp,
            partitions,
            visible_links,
        })
    }

    pub fn bsp(&self) -> &str {
        &self.bsp
    }

    pub fn partitions(&self) -> &[i64] {
        &self.partitions
    }

    pub fn visible_links(&self) -> &[(i64, i64)] {
        &self.visible_links
    }
}

#[derive(Clone, Debug)]
struct PartitionMeta {
    region_index: usize,
    degree1_visible: Vec<i64>,
}

/// Immutable degree-1 partition topology built by `Map.Build(channel)` + `channel.ConfirmRegions(1)`.
#[derive(Clone, Debug)]
pub struct Topology {
    partitions: HashMap<i64, PartitionMeta>,
    build_results: Vec<(String, bool)>,
    /// Region (bsp) name by region index.
    region_names: Vec<String>,
}

impl Topology {
    pub fn from_maps(maps: &[MapFile]) -> Self {
        struct RegionBuilder {
            sn_to_id: Vec<i64>,
            id_to_sn: HashMap<i64, usize>,
            links: Vec<Vec<usize>>,
        }
        impl RegionBuilder {
            fn new() -> Self {
                Self {
                    sn_to_id: Vec::new(),
                    id_to_sn: HashMap::new(),
                    links: Vec::new(),
                }
            }
            fn add_partition(&mut self, id: i64) -> usize {
                if let Some(&sn) = self.id_to_sn.get(&id) {
                    return sn;
                }
                let sn = self.sn_to_id.len();
                self.sn_to_id.push(id);
                self.id_to_sn.insert(id, sn);
                self.links.push(vec![sn]);
                sn
            }
            fn link(&mut self, lhs: i64, rhs: i64) -> bool {
                let (Some(&l), Some(&r)) = (self.id_to_sn.get(&lhs), self.id_to_sn.get(&rhs))
                else {
                    return false;
                };
                if !self.links[l].contains(&r) {
                    self.links[l].push(r);
                    if !self.links[r].contains(&l) {
                        self.links[r].push(l);
                    }
                }
                true
            }
        }

        let mut regions: Vec<RegionBuilder> = Vec::new();
        let mut region_by_name: HashMap<String, usize> = HashMap::new();
        let mut global_partitions: HashMap<i64, (usize, usize)> = HashMap::new();
        let mut build_results = Vec::with_capacity(maps.len());

        let add_region = |bsp: &str,
                          regions: &mut Vec<RegionBuilder>,
                          region_by_name: &mut HashMap<String, usize>|
         -> bool {
            if region_by_name.contains_key(bsp) {
                return false;
            }
            let idx = regions.len();
            regions.push(RegionBuilder::new());
            region_by_name.insert(bsp.to_owned(), idx);
            true
        };

        for map in maps {
            add_region(&map.bsp, &mut regions, &mut region_by_name);
            let r_idx = region_by_name[&map.bsp];
            let mut ok = true;
            for &pid in &map.partitions {
                if global_partitions.contains_key(&pid) {
                    ok = false;
                    break;
                }
                let sn = regions[r_idx].add_partition(pid);
                global_partitions.insert(pid, (r_idx, sn));
            }
            if ok {
                for &(lhs, rhs) in &map.visible_links {
                    if !regions[r_idx].link(lhs, rhs) {
                        ok = false;
                        break;
                    }
                }
            }
            build_results.push((map.bsp.clone(), ok));
        }

        // ConfirmRegions(1) -> BuildEmptyRegion("[Empty]", 0L)
        add_region("[Empty]", &mut regions, &mut region_by_name);
        if !global_partitions.contains_key(&0)
            && let Some(&empty_idx) = region_by_name.get("[Empty]")
        {
            let sn = regions[empty_idx].add_partition(0);
            global_partitions.insert(0, (empty_idx, sn));
        }

        let mut partitions = HashMap::with_capacity(global_partitions.len());
        for (r_idx, reg) in regions.iter().enumerate() {
            let max_sn = reg.sn_to_id.len();
            // Degree 0: visibilityTable[0, i, i] = (snIndex[i].ID != 0)
            let mut deg0 = vec![vec![false; max_sn]; max_sn];
            for (i, row) in deg0.iter_mut().enumerate() {
                if reg.sn_to_id[i] != 0 {
                    row[i] = true;
                }
            }
            // Degree 1
            for k in 0..max_sn {
                let mut visible_ids = Vec::new();
                for (l, _) in deg0.iter().enumerate() {
                    let vis = deg0[k][l] || reg.links[k].iter().any(|&linked| deg0[linked][l]);
                    if vis {
                        visible_ids.push(reg.sn_to_id[l]);
                    }
                }
                partitions.insert(
                    reg.sn_to_id[k],
                    PartitionMeta {
                        region_index: r_idx,
                        degree1_visible: visible_ids,
                    },
                );
            }
        }

        let mut region_names = vec![String::new(); regions.len()];
        for (name, idx) in region_by_name {
            if let Some(slot) = region_names.get_mut(idx) {
                *slot = name;
            }
        }
        Self {
            partitions,
            build_results,
            region_names,
        }
    }

    /// The region (bsp) holding `partition_id`.
    pub fn partition_region(&self, partition_id: i64) -> Option<&str> {
        let meta = self.partitions.get(&partition_id)?;
        self.region_names.get(meta.region_index).map(String::as_str)
    }

    pub fn build_results(&self) -> &[(String, bool)] {
        &self.build_results
    }

    pub fn neighbor_partitions(&self, partition_id: i64) -> &[i64] {
        self.partitions
            .get(&partition_id)
            .map_or(&[], |m| m.degree1_visible.as_slice())
    }

    pub fn contains_partition(&self, partition_id: i64) -> bool {
        self.partitions.contains_key(&partition_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComponentKind {
    Location,
    Look,
    Kinetics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutgoingPacket {
    pub category: i32,
    pub body: Vec<u8>,
}

struct PlayerComponents {
    cur_partition: i64,
    next_partition: i64,
    look_packet: OutgoingPacket,
    kinetics_packet: OutgoingPacket,
}

struct PartitionRuntime {
    observers: Vec<i64>,
    sight_slots: Vec<Option<(i64, ComponentKind)>>,
    sight_free: Vec<usize>,
}

impl PartitionRuntime {
    fn new() -> Self {
        Self {
            observers: Vec::new(),
            sight_slots: Vec::new(),
            sight_free: Vec::new(),
        }
    }

    fn contains_sight(&self, item: (i64, ComponentKind)) -> bool {
        self.sight_slots.contains(&Some(item))
    }

    fn add_sight(&mut self, item: (i64, ComponentKind)) {
        if self.contains_sight(item) {
            return;
        }
        if let Some(slot) = self.sight_free.pop() {
            self.sight_slots[slot] = Some(item);
        } else {
            self.sight_slots.push(Some(item));
        }
    }

    fn remove_sight(&mut self, item: (i64, ComponentKind)) {
        if let Some(pos) = self.sight_slots.iter().position(|&x| x == Some(item)) {
            self.sight_slots[pos] = None;
            self.sight_free.push(pos);
        }
    }

    fn sight_items(&self) -> Vec<(i64, ComponentKind)> {
        self.sight_slots.iter().flatten().copied().collect()
    }
}

/// Single channel instance matching `MMOServer.Channel` + `HeroesChannelServer.Player`.
pub struct Channel {
    topology: Topology,
    partitions: HashMap<i64, PartitionRuntime>,
    players: HashMap<i64, PlayerComponents>,
    observer_watching: HashMap<i64, i64>,
}

impl Channel {
    pub fn new(topology: Topology) -> Self {
        let mut partitions = HashMap::with_capacity(topology.partitions.len());
        for &pid in topology.partitions.keys() {
            partitions.insert(pid, PartitionRuntime::new());
        }
        Self {
            topology,
            partitions,
            players: HashMap::new(),
            observer_watching: HashMap::new(),
        }
    }

    pub fn member_count(&self) -> usize {
        self.players.len()
    }

    /// Members as (CID, current partition), by CID.
    pub fn members(&self) -> Vec<(i64, i64)> {
        let mut out: Vec<(i64, i64)> = self
            .players
            .iter()
            .map(|(cid, p)| (*cid, p.cur_partition))
            .collect();
        out.sort_unstable();
        out
    }

    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    fn encode_look(cid: i64, summary_bytes: &[u8]) -> Result<OutgoingPacket, MapPartitionError> {
        let decoded = character_list::decode(summary_bytes, DEFAULT_MAX_BODY)
            .map_err(|_| MapPartitionError::Wire)?
            .ok_or(MapPartitionError::Wire)?;
        let summary = decoded
            .characters
            .and_then(|mut c| if c.len() == 1 { c.pop() } else { None })
            .flatten()
            .ok_or(MapPartitionError::Wire)?;
        Self::encode_look_summary(cid, Some(&summary))
    }

    pub fn encode_look_summary(
        cid: i64,
        summary: Option<&Summary<'_>>,
    ) -> Result<OutgoingPacket, MapPartitionError> {
        let body = minimum_map::encode(
            Kind::NotifyLook,
            Some(&Message::NotifyLook {
                id: cid,
                look: summary.cloned(),
            }),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| MapPartitionError::Wire)?;
        Ok(OutgoingPacket {
            category: Kind::NotifyLook.category(),
            body: body.to_vec(),
        })
    }

    pub fn encode_action(
        cid: i64,
        action: ActionSync,
    ) -> Result<OutgoingPacket, MapPartitionError> {
        let body = minimum_map::encode(
            Kind::NotifyAction,
            Some(&Message::NotifyAction { id: cid, action }),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| MapPartitionError::Wire)?;
        Ok(OutgoingPacket {
            category: Kind::NotifyAction.category(),
            body: body.to_vec(),
        })
    }

    pub fn encode_disappeared(cid: i64) -> Result<OutgoingPacket, MapPartitionError> {
        let body = minimum_map::encode(
            Kind::Disappeared,
            Some(&Message::Disappeared { id: cid }),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| MapPartitionError::Wire)?;
        Ok(OutgoingPacket {
            category: Kind::Disappeared.category(),
            body: body.to_vec(),
        })
    }

    fn can_see(&self, origin: i64, target: i64) -> bool {
        self.topology.neighbor_partitions(origin).contains(&target)
    }

    fn region_of(&self, pid: i64) -> Option<usize> {
        self.topology.partitions.get(&pid).map(|m| m.region_index)
    }

    fn appear_message(&self, cid: i64, kind: ComponentKind) -> Option<OutgoingPacket> {
        let p = self.players.get(&cid)?;
        match kind {
            ComponentKind::Location => None,
            ComponentKind::Look => Some(p.look_packet.clone()),
            ComponentKind::Kinetics => Some(p.kinetics_packet.clone()),
        }
    }

    fn disappear_message(cid: i64, kind: ComponentKind) -> Option<OutgoingPacket> {
        match kind {
            ComponentKind::Location => Self::encode_disappeared(cid).ok(),
            ComponentKind::Look | ComponentKind::Kinetics => None,
        }
    }

    fn broadcast_partition(
        &self,
        pid: i64,
        pkt: &OutgoingPacket,
        out: &mut BTreeMap<i64, Vec<OutgoingPacket>>,
    ) {
        if let Some(part) = self.partitions.get(&pid) {
            for &obs in &part.observers {
                out.entry(obs).or_default().push(pkt.clone());
            }
        }
    }

    fn broadcast_degree1(
        &self,
        origin: i64,
        pkt: &OutgoingPacket,
        out: &mut BTreeMap<i64, Vec<OutgoingPacket>>,
    ) {
        for &vis in self.topology.neighbor_partitions(origin) {
            self.broadcast_partition(vis, pkt, out);
        }
    }

    fn add_sight(&mut self, pid: i64, item: (i64, ComponentKind)) {
        if let Some(part) = self.partitions.get_mut(&pid) {
            part.add_sight(item);
        }
    }

    fn remove_sight(&mut self, pid: i64, item: (i64, ComponentKind)) {
        if let Some(part) = self.partitions.get_mut(&pid) {
            part.remove_sight(item);
        }
    }

    /// Executes `Player.Enter(pid, look, action)`:
    /// 1. `AddComponent(MakeLocation(cid, 1, pid))`
    /// 2. `AddComponent(new Look(cid, look))`
    /// 3. `AddComponent(new Kinetics(cid, action))`
    /// 4. `AddObserver(pid, player)`
    pub fn enter_with_look_packet(
        &mut self,
        cid: i64,
        pid: i64,
        look_packet: OutgoingPacket,
        action: ActionSync,
    ) -> Result<BTreeMap<i64, Vec<OutgoingPacket>>, MapPartitionError> {
        if self.players.contains_key(&cid) {
            self.leave(cid);
        }
        let cur = if self.topology.contains_partition(pid) {
            pid
        } else {
            -1
        };
        let kinetics_packet = Self::encode_action(cid, action)?;
        self.players.insert(
            cid,
            PlayerComponents {
                cur_partition: cur,
                next_partition: cur,
                look_packet: look_packet.clone(),
                kinetics_packet: kinetics_packet.clone(),
            },
        );
        let mut out = BTreeMap::new();
        let vis: Vec<i64> = self.topology.neighbor_partitions(cur).to_vec();

        // 1. AddComponent(Location) -> ProcessPartitionAppear(Location)
        for &p in &vis {
            self.add_sight(p, (cid, ComponentKind::Location));
        }
        // 2. AddComponent(Look) -> BroadcastAppear(Look) + AddSightComponent(Look)
        self.broadcast_degree1(cur, &look_packet, &mut out);
        for &p in &vis {
            self.add_sight(p, (cid, ComponentKind::Look));
        }
        // 3. AddComponent(Kinetics) -> BroadcastAppear(Kinetics) + AddSightComponent(Kinetics)
        self.broadcast_degree1(cur, &kinetics_packet, &mut out);
        for &p in &vis {
            self.add_sight(p, (cid, ComponentKind::Kinetics));
        }
        // 4. AddObserver(pid, player)
        self.add_observer_inner(cid, pid, &mut out);
        Ok(out)
    }

    pub fn enter_from_character_list(
        &mut self,
        cid: i64,
        pid: i64,
        character_list_body: &Zeroizing<Vec<u8>>,
        action: ActionSync,
    ) -> Result<BTreeMap<i64, Vec<OutgoingPacket>>, MapPartitionError> {
        let look_packet = Self::encode_look(cid, character_list_body)?;
        self.enter_with_look_packet(cid, pid, look_packet, action)
    }

    fn add_observer_inner(
        &mut self,
        cid: i64,
        pid: i64,
        out: &mut BTreeMap<i64, Vec<OutgoingPacket>>,
    ) -> bool {
        let Some(part) = self.partitions.get_mut(&pid) else {
            self.observer_watching.insert(cid, 0);
            return false;
        };
        if part.observers.contains(&cid) {
            return false;
        }
        part.observers.push(cid);
        self.observer_watching.insert(cid, pid);
        let sight = part.sight_items();
        for (owner, kind) in sight {
            if let Some(pkt) = self.appear_message(owner, kind) {
                out.entry(cid).or_default().push(pkt);
            }
        }
        true
    }

    /// Executes `Player.Action(action)` (`ApplyModifier(new KineticsChanged(cid, action))`).
    pub fn update_action(
        &mut self,
        cid: i64,
        action: ActionSync,
    ) -> Result<BTreeMap<i64, Vec<OutgoingPacket>>, MapPartitionError> {
        let mut out = BTreeMap::new();
        let pkt = Self::encode_action(cid, action)?;
        let Some(player) = self.players.get_mut(&cid) else {
            return Ok(out);
        };
        player.kinetics_packet = pkt.clone();
        let cur = player.cur_partition;
        self.broadcast_degree1(cur, &pkt, &mut out);
        Ok(out)
    }

    /// Executes `Player.UpdateLook(summary)` (`ApplyModifier(new UpdateLook(cid, summary))`):
    /// the Look component takes the new NotifyLook and `NotifyModified` sends it, as its
    /// DifferenceMessage, to the observers of the visible partitions.
    pub fn update_look(
        &mut self,
        cid: i64,
        look_packet: OutgoingPacket,
    ) -> BTreeMap<i64, Vec<OutgoingPacket>> {
        let mut out = BTreeMap::new();
        let Some(player) = self.players.get_mut(&cid) else {
            return out;
        };
        player.look_packet = look_packet.clone();
        let cur = player.cur_partition;
        self.broadcast_degree1(cur, &look_packet, &mut out);
        out
    }

    /// Executes `Player.Move(target_pid)`:
    /// 1. `ApplyModifier(MakeMove(cid, target_pid))`
    /// 2. `MoveObserver(target_pid, player)`
    pub fn move_partition(
        &mut self,
        cid: i64,
        target_pid: i64,
    ) -> Result<BTreeMap<i64, Vec<OutgoingPacket>>, MapPartitionError> {
        let mut out = BTreeMap::new();
        // 1. ApplyModifier(MakeMove(cid, target_pid))
        if self.topology.contains_partition(target_pid)
            && let Some(player) = self.players.get_mut(&cid)
            && player.next_partition != target_pid
        {
            let cur = player.cur_partition;
            let next = target_pid;
            player.next_partition = next;
            let look_pkt = player.look_packet.clone();
            let kin_pkt = player.kinetics_packet.clone();
            let dis_pkt = Self::encode_disappeared(cid)?;

            if cur != next {
                let same_region =
                    self.region_of(cur).is_some() && self.region_of(cur) == self.region_of(next);
                let cur_vis: Vec<i64> = self.topology.neighbor_partitions(cur).to_vec();
                let next_vis: Vec<i64> = self.topology.neighbor_partitions(next).to_vec();
                let kinds = [
                    ComponentKind::Location,
                    ComponentKind::Look,
                    ComponentKind::Kinetics,
                ];

                if !same_region {
                    // BroadcastMove across different regions
                    self.broadcast_degree1(cur, &dis_pkt, &mut out);
                    self.broadcast_degree1(next, &look_pkt, &mut out);
                    self.broadcast_degree1(next, &kin_pkt, &mut out);
                    // ProcessPartitionMove across different regions
                    for &p in &cur_vis {
                        for kind in kinds {
                            self.remove_sight(p, (cid, kind));
                        }
                    }
                    for &p in &next_vis {
                        for kind in kinds {
                            self.add_sight(p, (cid, kind));
                        }
                    }
                } else {
                    // BroadcastMove within same region
                    for &item3 in &cur_vis {
                        if self.can_see(item3, cur) && !self.can_see(item3, next) {
                            self.broadcast_partition(item3, &dis_pkt, &mut out);
                        }
                    }
                    for &item4 in &next_vis {
                        if self.can_see(next, item4) && !self.can_see(item4, cur) {
                            self.broadcast_partition(item4, &look_pkt, &mut out);
                            self.broadcast_partition(item4, &kin_pkt, &mut out);
                        }
                    }
                    // ProcessPartitionMove within same region
                    for &p in &cur_vis {
                        if !next_vis.contains(&p) {
                            for kind in kinds {
                                self.remove_sight(p, (cid, kind));
                            }
                        }
                    }
                    for &p in &next_vis {
                        if !cur_vis.contains(&p) {
                            for kind in kinds {
                                self.add_sight(p, (cid, kind));
                            }
                        }
                    }
                }
            }
            if let Some(player) = self.players.get_mut(&cid) {
                player.cur_partition = player.next_partition;
            }
        }

        // 2. MoveObserver(target_pid, player)
        let watching = self.observer_watching.get(&cid).copied().unwrap_or(0);
        if !self.partitions.contains_key(&watching) {
            self.add_observer_inner(cid, target_pid, &mut out);
            return Ok(out);
        }
        if !self.partitions.contains_key(&target_pid) {
            return Ok(out);
        }
        let Some(old_part) = self.partitions.get_mut(&watching) else {
            return Ok(out);
        };
        let Some(obs_pos) = old_part.observers.iter().position(|&x| x == cid) else {
            return Ok(out);
        };
        old_part.observers.remove(obs_pos);
        let old_sight = old_part.sight_items();

        let Some(new_part) = self.partitions.get_mut(&target_pid) else {
            return Ok(out);
        };
        if new_part.observers.contains(&cid) {
            return Ok(out);
        }
        new_part.observers.push(cid);
        self.observer_watching.insert(cid, target_pid);
        let new_sight = new_part.sight_items();

        for item in &old_sight {
            if !new_sight.contains(item)
                && let Some(pkt) = Self::disappear_message(item.0, item.1)
            {
                out.entry(cid).or_default().push(pkt);
            }
        }
        for item in &new_sight {
            if !old_sight.contains(item)
                && let Some(pkt) = self.appear_message(item.0, item.1)
            {
                out.entry(cid).or_default().push(pkt);
            }
        }
        Ok(out)
    }

    /// Executes `Player.LeaveChannel()`:
    /// 1. `RemoveObserver(player)`
    /// 2. `RemoveComponent` for `[Location, Look, Kinetics]`.
    pub fn leave(&mut self, cid: i64) -> BTreeMap<i64, Vec<OutgoingPacket>> {
        let mut out = BTreeMap::new();
        if let Some(watching) = self.observer_watching.remove(&cid)
            && let Some(part) = self.partitions.get_mut(&watching)
            && let Some(pos) = part.observers.iter().position(|&x| x == cid)
        {
            part.observers.remove(pos);
        }
        if let Some(player) = self.players.remove(&cid) {
            let cur = player.cur_partition;
            if let Ok(dis_pkt) = Self::encode_disappeared(cid) {
                self.broadcast_degree1(cur, &dis_pkt, &mut out);
            }
            let vis: Vec<i64> = self.topology.neighbor_partitions(cur).to_vec();
            for p in vis {
                for kind in [
                    ComponentKind::Location,
                    ComponentKind::Look,
                    ComponentKind::Kinetics,
                ] {
                    self.remove_sight(p, (cid, kind));
                }
            }
        }
        out
    }

    pub fn watching_partition(&self, cid: i64) -> Option<i64> {
        self.observer_watching.get(&cid).copied()
    }
}
