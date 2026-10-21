//! ItemTraitExtension.Stack for a positive moving count, after IsStackableWith.
//! Returns the new (occupant, moving) counts, or None when the source returns false.

/// `occupant` is `oldItem` (the slot owner); `moving` is `newItem`.
pub fn stack(occupant: i32, moving: i32, max_stack: i32) -> Option<(i32, i32)> {
    if moving == 0 {
        return Some((occupant, moving));
    }
    // IsMaxStack: Num >= MaxStack.
    if occupant >= max_stack || moving < 0 {
        return None;
    }
    let amount = max_stack.checked_sub(occupant)?.min(moving);
    if amount > 0 {
        Some((occupant.checked_add(amount)?, moving - amount))
    } else {
        None
    }
}
