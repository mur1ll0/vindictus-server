use vindictus_server::domain::item_stack::stack;

#[test]
fn stack_moves_up_to_max_and_reports_source_false_paths() {
    // Partial: the occupant fills to MaxStack and the remainder stays.
    assert_eq!(stack(90, 20, 99), Some((99, 11)));
    // Complete: the moving item reaches zero and is deleted by the caller.
    assert_eq!(stack(10, 20, 99), Some((30, 0)));
    // IsMaxStack (Num >= MaxStack) returns false: SwapSlot swaps instead.
    assert_eq!(stack(99, 1, 99), None);
    assert_eq!(stack(120, 1, 99), None);
    // Num == 0 returns true without changes.
    assert_eq!(stack(5, 0, 99), Some((5, 0)));
    assert_eq!(stack(0, 1, 1), Some((1, 0)));
}
