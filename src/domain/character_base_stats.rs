//! Seven persisted base stats, before in-memory overrides and equipment effects.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BaseStats([i32; 7]);
impl BaseStats {
    /// Source GetCharacterStats inserts LUCK=100; CharacterStats.Add is unchecked Int32.
    /// Title contribution must be recovered by the caller, never inferred as zero here.
    pub fn compose(class_stats: [i32; 6], titles: [i32; 7]) -> Self {
        let [s, d, i, w, hp, stamina] = class_stats;
        let mut result = [s, d, i, w, 100, hp, stamina];
        for (value, bonus) in result.iter_mut().zip(titles) {
            *value = value.wrapping_add(bonus);
        }
        Self(result)
    }
    pub fn values(&self) -> &[i32; 7] {
        &self.0
    }
}
