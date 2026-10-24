//! SkillEnhance: CharacterContents rows, the per-skill enhance row and the
//! SkillEnhanceSession rules (MakeSkillEnhanceSession, the material processors, RollDice
//! and the OnDiceSuccess/OnDiceFail item and enhance decisions).

/// SkillEnhanceInfo columns the item service reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub group: i32,
    pub key: i32,
    pub item_class: Option<String>,
    pub max_success_ratio: i32,
    pub max_durability: i32,
    pub repair_value: i32,
}

/// SkillEnhanceMaterialInfo: Type 0 raises SuccessRatio, 1 the MaxSuccessRatio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Material {
    pub item_class: String,
    pub kind: u8,
    pub min: i32,
    pub max: i32,
}

/// SkillEnhance.EnhanceType.
pub const TYPE_NONE: u8 = 0;
pub const TYPE_SUCCESS: u8 = 1;
pub const TYPE_PENALTY: u8 = 2;

/// A dbo.SkillEnhance row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enhance {
    pub skill_row_id: i64,
    pub group: i32,
    pub key: i32,
    pub kind: u8,
    pub reduce_durability: i32,
    pub max_durability_bonus: i32,
}

/// SkillEnhanceResult names (sent as GameUI_Heroes_SkillEnhance_{0}).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    GeneralCannotFindSession,
    MakeSessionFailFeatureOff,
    MakeSessionFailCannotFindSkillData,
    MakeSessionFailCannotFindEnhanceStone,
    MakeSessionFailCannotFindSkillEnhanceInfo,
    MakeSessionFailNewEnhanceKeyLowerThanEnhanceDataKey,
    MakeSessionFailNotAvailableEnhanceGroup,
    UseErgBiggerThanMaxRatio,
    UseErgAlreadyHasAdditionalMaxRatio,
    UseErgCannotDestroyMultipleItems,
    UseErgNotImplementation,
    UseErgNotEnoughItem,
}
impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Self::GeneralCannotFindSession => "General_CannotFindSession",
            Self::MakeSessionFailFeatureOff => "MakeSessionFail_FeatureOff",
            Self::MakeSessionFailCannotFindSkillData => "MakeSessionFail_CannotFindSkillData",
            Self::MakeSessionFailCannotFindEnhanceStone => "MakeSessionFail_CannotFindEnhanceStone",
            Self::MakeSessionFailCannotFindSkillEnhanceInfo => {
                "MakeSessionFail_CannotFindSkillEnhanceInfo"
            }
            Self::MakeSessionFailNewEnhanceKeyLowerThanEnhanceDataKey => {
                "MakeSessionFail_NewEnhanceKeyLowerThanEnhanceDataKey"
            }
            Self::MakeSessionFailNotAvailableEnhanceGroup => {
                "MakeSessionFail_NotAvailableEnhanceGroup"
            }
            Self::UseErgBiggerThanMaxRatio => "UseErg_BiggerThanMaxRatio",
            Self::UseErgAlreadyHasAdditionalMaxRatio => "UseErg_AlreadyHasAdditionalMaxRatio",
            Self::UseErgCannotDestroyMultipleItems => "UseErg_CannotDestroyMultipleItems",
            Self::UseErgNotImplementation => "UseErg_NotImplementation",
            Self::UseErgNotEnoughItem => "UseErg_NotEnough_Item",
        }
    }
}

/// SkillEnhanceSession.MaxPercentage.
pub const MAX_PERCENTAGE: i32 = 100;

/// The open SkillEnhanceSession of a character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub skill: String,
    pub skill_row_id: i64,
    pub stone_item_id: i64,
    pub info: Info,
    pub success_ratio: i32,
    additional_max: i32,
}
impl Session {
    pub fn max_success_ratio(&self) -> i32 {
        self.info
            .max_success_ratio
            .wrapping_add(self.additional_max)
    }
    pub fn additional_max(&self) -> i32 {
        self.additional_max
    }
    /// AdditionalMaxSuccessRatio setter: the total stays within (0, MaxPercentage].
    fn set_additional_max(&mut self, value: i32) {
        self.additional_max = value;
        if self.max_success_ratio() > MAX_PERCENTAGE {
            self.additional_max = MAX_PERCENTAGE.wrapping_sub(self.info.max_success_ratio);
        } else if self.max_success_ratio() <= 0 {
            self.additional_max = 0;
        }
    }
}

/// What MakeSkillEnhanceSession reads from the character and the contents.
pub struct StartFacts<'a> {
    pub feature_on: bool,
    /// The skill's RowID and its current enhance row.
    pub skill: Option<(i64, Option<Enhance>)>,
    /// The stone item ID and class.
    pub stone: Option<(i64, &'a str)>,
    /// GetSkillEnhanceInfo(stone class).
    pub info: Option<&'a Info>,
    /// SkillInfo.GetAvailableSkillEnhanceGroup (None: unknown SkillInfo).
    pub groups: Option<Vec<i32>>,
}

/// SkillEnhanceSession.MakeSkillEnhanceSession.
pub fn start(skill: &str, facts: StartFacts<'_>) -> Result<Session, Outcome> {
    if !facts.feature_on {
        return Err(Outcome::MakeSessionFailFeatureOff);
    }
    let (row_id, enhance) = facts
        .skill
        .ok_or(Outcome::MakeSessionFailCannotFindSkillData)?;
    let (stone_item_id, _) = facts
        .stone
        .ok_or(Outcome::MakeSessionFailCannotFindEnhanceStone)?;
    let info = facts
        .info
        .ok_or(Outcome::MakeSessionFailCannotFindSkillEnhanceInfo)?;
    if !facts.groups.is_some_and(|g| g.contains(&info.group)) {
        return Err(Outcome::MakeSessionFailNotAvailableEnhanceGroup);
    }
    if let Some(e) = enhance
        && e.kind == TYPE_PENALTY
        && e.key > info.key
    {
        return Err(Outcome::MakeSessionFailNewEnhanceKeyLowerThanEnhanceDataKey);
    }
    Ok(Session {
        skill: skill.to_owned(),
        skill_row_id: row_id,
        stone_item_id,
        info: info.clone(),
        success_ratio: 0,
        additional_max: 0,
    })
}

/// SkillInfo.GetAvailableSkillEnhanceGroup: ';'-separated int.TryParse values.
pub fn parse_groups(text: Option<&str>) -> Vec<i32> {
    text.unwrap_or("")
        .split(';')
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect()
}

/// The material processor checks (CanIncreaseRatio) for UseCount items.
pub fn can_use_material(
    session: &Session,
    material: &Material,
    use_count: i32,
) -> Result<(), Outcome> {
    match material.kind {
        0 if session.success_ratio >= session.max_success_ratio() => {
            Err(Outcome::UseErgBiggerThanMaxRatio)
        }
        0 => Ok(()),
        1 if session.additional_max != 0 => Err(Outcome::UseErgAlreadyHasAdditionalMaxRatio),
        1 if use_count != 1 => Err(Outcome::UseErgCannotDestroyMultipleItems),
        1 => Ok(()),
        _ => Err(Outcome::UseErgNotImplementation),
    }
}

/// IncreaseRatio after the items were destroyed (RandomObj.Next(min, max + 1)).
pub fn use_material(
    session: &mut Session,
    material: &Material,
    use_count: i32,
    random: &mut dyn FnMut(i32, i32) -> Option<i32>,
) -> Option<()> {
    match material.kind {
        0 => {
            for _ in 0..use_count.max(0) {
                let add = random(material.min, material.max.wrapping_add(1))?;
                session.success_ratio = session.success_ratio.wrapping_add(add);
                if session.success_ratio > session.max_success_ratio() {
                    session.success_ratio = session.max_success_ratio();
                }
            }
            Some(())
        }
        1 => {
            let value = if material.min == material.max {
                material.min
            } else {
                random(material.min, material.max.wrapping_add(1))?
            };
            session.set_additional_max(value);
            Some(())
        }
        _ => None,
    }
}

/// SkillEnhanceConst rune classes (case-insensitive).
pub fn is_premium_rune(class: &str) -> bool {
    ["skill_enhance_rune_gift_lv2", "skill_enhance_rune_cash_lv2"]
        .iter()
        .any(|r| r.eq_ignore_ascii_case(class))
}
pub fn is_rune(class: &str) -> bool {
    is_premium_rune(class)
        || ["skill_enhance_rune_gift_lv1", "skill_enhance_rune_cash_lv1"]
            .iter()
            .any(|r| r.eq_ignore_ascii_case(class))
}

/// The item and enhance decisions of OnDiceSuccess / OnDiceFail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roll {
    pub success: bool,
    /// The first rune of AdditionalItemIDs loses one (premium only on success).
    pub consume_rune: bool,
    pub consume_stone: bool,
    /// A protected copy of the stone (VALUE SKILLENHANCE_PROTECTED, Arg 1) is given.
    pub give_protected_stone: bool,
    /// Some(row): InsertOrUpdateSkillEnhance; None: RemoveSkillEnhance.
    pub enhance: Option<Enhance>,
    pub additional_destroyed: bool,
    pub stone_protected: bool,
}

/// RollDice (Next(MaxPercentage) < SuccessRatio) then the outcome. `rune`: the class of
/// the first rune among AdditionalItemIDs; `stone_protected`: HasSkillEnhanceProtected.
pub fn roll(session: &Session, dice: i32, rune: Option<&str>, stone_protected: bool) -> Roll {
    let row = |kind| Enhance {
        skill_row_id: session.skill_row_id,
        group: session.info.group,
        key: session.info.key,
        kind,
        reduce_durability: 0,
        max_durability_bonus: 0,
    };
    if dice < session.success_ratio {
        return Roll {
            success: true,
            consume_rune: rune.is_some_and(is_premium_rune),
            consume_stone: true,
            give_protected_stone: false,
            enhance: Some(row(TYPE_SUCCESS)),
            additional_destroyed: false,
            stone_protected: false,
        };
    }
    let premium = rune.is_some_and(is_premium_rune);
    let has_rune = rune.is_some();
    Roll {
        success: false,
        consume_rune: has_rune,
        consume_stone: !premium || !stone_protected,
        give_protected_stone: has_rune && !stone_protected,
        enhance: (!has_rune).then(|| row(TYPE_PENALTY)),
        additional_destroyed: has_rune,
        stone_protected: premium || (!stone_protected && has_rune),
    }
}
