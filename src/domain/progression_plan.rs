//! The panel's "reach a level" plan (docs/future/painel-personagem-e-busca.md, section 1):
//! the mission quests a character of that level would have cleared. A quest enters when it
//! is feature-enabled, a regular quest (QuestSet below 999, not a hunting quest), its
//! QuestInfo.Level is at most the level and one of its difficulties opens at the level
//! (QuestDifficultyInfo.MinLevel, the lowest such difficulty is used); a quest whose
//! QuestConstraintInfo requires another quest enters only with that quest. Storylines are
//! not completed by the plan: their conditions read the quest progress
//! (`quest_cleared`), so they advance when the character talks to the NPCs.

/// QuestSet of test, PvP and event boards (999 and above).
pub const SPECIAL_QUEST_SET: i32 = 999;

/// What the content says about one quest.
#[derive(Clone, Debug)]
pub struct QuestFacts {
    pub id: String,
    /// QuestInfo.Level; None when the mission contents lack the quest.
    pub level: Option<i32>,
    pub quest_set: i32,
    pub hunting: bool,
    pub enabled: bool,
    /// Feature-enabled (Difficulty, MinLevel).
    pub difficulties: Vec<(i32, i32)>,
    /// QuestConstraintInfo: RequiredQuestID.
    pub requires: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct PlannedQuest {
    pub id: String,
    pub level: i32,
    pub difficulty: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct SkippedQuest {
    pub id: String,
    /// feature, special, no_mission, difficulty or requirement.
    pub reason: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct Plan {
    pub level: i32,
    pub quests: Vec<PlannedQuest>,
    /// Quests up to the level left out, with the reason.
    pub skipped: Vec<SkippedQuest>,
}

/// The quests of `facts` (provider order) that the plan completes for `level`.
pub fn plan(level: i32, facts: &[QuestFacts]) -> Plan {
    let mut out = Plan {
        level,
        ..Plan::default()
    };
    let mut candidates: Vec<(&QuestFacts, PlannedQuest)> = Vec::new();
    for f in facts {
        let Some(quest_level) = f.level else {
            out.skipped.push(skip(f, "no_mission"));
            continue;
        };
        if quest_level > level {
            continue;
        }
        if !f.enabled {
            out.skipped.push(skip(f, "feature"));
            continue;
        }
        if f.quest_set >= SPECIAL_QUEST_SET || f.hunting {
            out.skipped.push(skip(f, "special"));
            continue;
        }
        let Some(difficulty) = f
            .difficulties
            .iter()
            .filter(|(_, min)| *min <= level)
            .map(|(d, _)| *d)
            .min()
        else {
            out.skipped.push(skip(f, "difficulty"));
            continue;
        };
        candidates.push((
            f,
            PlannedQuest {
                id: f.id.clone(),
                level: quest_level,
                difficulty,
            },
        ));
    }
    // A quest stays only while every quest it requires stays (fixed point).
    loop {
        let kept: std::collections::HashSet<&str> =
            candidates.iter().map(|(f, _)| f.id.as_str()).collect();
        let (stay, gone): (Vec<_>, Vec<_>) = candidates
            .into_iter()
            .partition(|(f, _)| f.requires.iter().all(|r| kept.contains(r.as_str())));
        let done = gone.is_empty();
        out.skipped
            .extend(gone.iter().map(|(f, _)| skip(f, "requirement")));
        candidates = stay;
        if done {
            break;
        }
    }
    out.quests = candidates.into_iter().map(|(_, p)| p).collect();
    out
}

fn skip(f: &QuestFacts, reason: &'static str) -> SkippedQuest {
    SkippedQuest {
        id: f.id.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quest(id: &str, level: i32, min: i32) -> QuestFacts {
        QuestFacts {
            id: id.into(),
            level: Some(level),
            quest_set: 1,
            hunting: false,
            enabled: true,
            difficulties: vec![(1, min), (2, min), (3, 1)],
            requires: Vec::new(),
        }
    }

    #[test]
    fn quests_up_to_the_level_with_their_lowest_open_difficulty() {
        let mut hard = quest("hard_only", 10, 50);
        hard.difficulties = vec![(4, 50)];
        let facts = [
            quest("gnoll_scout", 1, 1),
            quest("bear", 40, 40),
            quest("later", 41, 41),
            hard,
            QuestFacts {
                quest_set: 1000,
                ..quest("fishing", 1, 1)
            },
            QuestFacts {
                enabled: false,
                ..quest("event", 1, 1)
            },
            QuestFacts {
                level: None,
                ..quest("lost", 1, 1)
            },
        ];
        let plan = plan(40, &facts);
        let ids: Vec<_> = plan
            .quests
            .iter()
            .map(|q| (q.id.as_str(), q.difficulty))
            .collect();
        assert_eq!(ids, [("gnoll_scout", 1), ("bear", 1)]);
        let skipped: Vec<_> = plan
            .skipped
            .iter()
            .map(|s| (s.id.as_str(), s.reason))
            .collect();
        assert_eq!(
            skipped,
            [
                ("hard_only", "difficulty"),
                ("fishing", "special"),
                ("event", "feature"),
                ("lost", "no_mission")
            ]
        );
    }

    #[test]
    fn requirements_follow_the_quests_that_stay() {
        let mut second = quest("second", 5, 1);
        second.requires = vec!["first".into()];
        let mut third = quest("third", 6, 1);
        third.requires = vec!["second".into()];
        let mut orphan = quest("orphan", 6, 1);
        orphan.requires = vec!["above".into()];
        let facts = [
            quest("first", 50, 1),
            second,
            third,
            orphan,
            quest("free", 2, 1),
        ];
        // "first" is above the level: the chain behind it leaves too.
        let plan = plan(10, &facts);
        let ids: Vec<_> = plan.quests.iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["free"]);
        assert_eq!(plan.skipped.len(), 3);
        assert!(plan.skipped.iter().all(|s| s.reason == "requirement"));
    }
}
