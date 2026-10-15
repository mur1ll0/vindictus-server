//! Recovered equipment durability state and the durability slice of Item.Initialize.
//! Call only after effective attribute stats and equipment colors have been initialized.
use super::item_stats::ItemStat;

/// Equipment-only state: the original class maximum controls HasDurability even
/// when attributes change Stat.MaxDurability. Values are private source data.
pub struct EquipmentDurability {
    source_maximum: i32,
    stat_maximum: i32,
    reduced: i32,
    bonus: i32,
}
impl EquipmentDurability {
    /// Rehydrate exact persisted fields without correcting them on read.
    pub fn load(equipment_base: &ItemStat, stat: &ItemStat, reduced: i32, bonus: i32) -> Self {
        Self {
            source_maximum: equipment_base.fields()[0],
            stat_maximum: stat.fields()[0],
            reduced,
            bonus,
        }
    }
    pub fn has_durability(&self) -> bool {
        self.source_maximum > 0
    }
    pub fn maximum(&self) -> i32 {
        if self.has_durability() {
            self.stat_maximum.wrapping_add(self.bonus)
        } else {
            0
        }
    }
    pub fn current(&self) -> i32 {
        let maximum = self.maximum();
        if maximum == 0 {
            0
        } else {
            maximum.wrapping_sub(self.reduced).max(0)
        }
    }
    pub fn reduced(&self) -> i32 {
        self.reduced
    }
    pub fn bonus(&self) -> i32 {
        self.bonus
    }
    /// Returns whether Item.Update(EquipmentStat) must be invoked. The source
    /// always updates here, including an unchanged value. The second condition
    /// compares the original request, so a negative feature limit is significant.
    pub fn set_maximum(&mut self, value: i32, limit: i32) -> bool {
        let mut corrected = value;
        if value < 0 {
            corrected = 0;
        }
        if value > limit {
            corrected = limit;
        }
        self.bonus = corrected.wrapping_sub(self.stat_maximum);
        true
    }
    /// The setter compares an unclamped-to-maximum expression, even when the
    /// getter returns zero because maximum is zero. Preserve that asymmetry.
    pub fn set_current(&mut self, value: i32) -> bool {
        let current = self.maximum().wrapping_sub(self.reduced).max(0);
        if current == value {
            return false;
        }
        self.reduced = self.maximum().wrapping_sub(value).max(0);
        true
    }
}
/// A missing equipment row skips this original Initialize branch. The feature
/// integer is supplied explicitly; no default limit or host configuration is used.
/// The returned count is the number of EquipmentStat update calls to replay.
/// This does not perform those updates, construct colors or assert Item.IsValid.
pub fn initialize(
    equipment_base: Option<&ItemStat>,
    stat: &ItemStat,
    requested_bonus: i32,
    requested_reduced: i32,
    limit: i32,
) -> Option<(EquipmentDurability, usize)> {
    let mut state = EquipmentDurability::load(equipment_base?, stat, 0, 0);
    let mut updates =
        usize::from(state.set_maximum(state.stat_maximum.wrapping_add(requested_bonus), limit));
    updates += usize::from(state.set_current(state.maximum().wrapping_sub(requested_reduced)));
    Some((state, updates))
}
