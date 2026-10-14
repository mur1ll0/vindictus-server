//! Recovered normal CharacterMaker slot limit; no slot reservation or default feature value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreationSlots {
    pub visible: i32,
    pub purchased: i32,
    pub premium: i32,
}
impl CreationSlots {
    /// Original unchecked int32 addition; max_free is the effective non-cheat feature value.
    pub fn at_limit(self, max_free: i32) -> bool {
        self.visible
            >= max_free
                .wrapping_add(self.purchased)
                .wrapping_add(self.premium)
    }
}
