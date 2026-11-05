use touchstone_model::{ArmourType, ItemType, SpellTargets, WornSlot};

// Expected values: modMMudFunc GetItemTypeEnum / GetArmourTypeEnum / GetWornTypeEnum /
// GetSpellTargetsEnum.
#[test]
fn codes_match_mme_labels() {
    assert_eq!(ItemType::from_code(8).unwrap().label(), "Container");
    assert_eq!(ItemType::from_code(11), None);
    assert_eq!(ItemType::from_code(-1), None);
    for c in 3..=6 {
        assert_eq!(ArmourType::from_code(c), Some(ArmourType::Leather));
    }
    assert_eq!(WornSlot::from_code(13), Some(WornSlot::Finger));
    assert_eq!(WornSlot::from_code(17), Some(WornSlot::Wrist));
    assert_eq!(WornSlot::from_code(20), None);
    assert_eq!(SpellTargets::from_code(11).unwrap().label(), "Full Area");
}
