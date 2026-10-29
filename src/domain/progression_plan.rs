//! The panel's "reach a level" plan (docs/future/painel-personagem-e-busca.md, section 1):
//! the mission quests a character of that level would have cleared. A quest enters when it
//! is feature-enabled, a regular quest (QuestSet below 999, not a hunting quest), its
//! QuestInfo.Level is at most the level and one of its difficulties opens at the level
//! (QuestDifficultyInfo.MinLevel, the lowest such difficulty is used); a quest whose
//! QuestConstraintInfo requires another quest enters only with that quest.
//!
//! The main story (StoryLineInfo.Category MainStream*) follows with `story_plan`: from the
//! main storylines auto-given up to the level, each storyline whose awaited quests
//! (`quest_cleared`, `QuestCleared`) are all cleared is completed and the storylines it starts
//! are walked; a main storyline still waiting for a quest, and every other storyline a
//! completed one starts, is opened at its initial phase, so the story goes on in the game from
//! there. Side, equipment and skill storylines are never completed.

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

/// What the content says about one storyline.
#[derive(Clone, Debug)]
pub struct StoryFacts<'a> {
    pub id: &'a str,
    /// StoryLineInfo.Category starts with MainStream.
    pub main: bool,
    /// NPCTalkInfo.IsStoryEnable.
    pub enabled: bool,
    /// StoryContents.IsAllowedTo the character's class.
    pub allowed: bool,
    pub auto_give_level: i32,
    pub reward_title: i32,
    pub reward_skill: Option<&'a str>,
    pub links: &'a super::npc_script::StoryLinks,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct StoryPlan {
    /// Main storylines completed, in walk order.
    pub complete: Vec<String>,
    /// Storylines opened at their initial phase: the next main ones and the side ones the
    /// completed storylines start.
    pub open: Vec<String>,
    /// Quests the completed storylines give (owned, not cleared, unless cleared already).
    pub give_quests: Vec<String>,
    pub titles: Vec<i32>,
    pub skills: Vec<String>,
}

/// The main story a character of `level` would have played, given the cleared quests.
pub fn story_plan(
    level: i32,
    lines: &[StoryFacts<'_>],
    cleared: &dyn Fn(&str) -> bool,
) -> StoryPlan {
    let by_id: std::collections::HashMap<&str, &StoryFacts<'_>> =
        lines.iter().map(|l| (l.id, l)).collect();
    let mut out = StoryPlan::default();
    let mut seen = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<&str> = lines
        .iter()
        .filter(|l| l.main && (1..=level).contains(&l.auto_give_level))
        .map(|l| l.id)
        .collect();
    let add = |list: &mut Vec<String>, v: &str| {
        if !list.iter().any(|x| x == v) {
            list.push(v.to_owned());
        }
    };
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        let Some(line) = by_id.get(id) else {
            continue;
        };
        if !line.enabled || !line.allowed {
            continue;
        }
        if !line.main || !line.links.requires.iter().all(|q| cleared(q)) {
            out.open.push(id.to_owned());
            continue;
        }
        out.complete.push(id.to_owned());
        for q in &line.links.gives {
            add(&mut out.give_quests, q);
        }
        for s in line
            .links
            .skills
            .iter()
            .map(String::as_str)
            .chain(line.reward_skill)
        {
            add(&mut out.skills, s);
        }
        for t in line
            .links
            .titles
            .iter()
            .copied()
            .chain((line.reward_title != 0).then_some(line.reward_title))
        {
            if !out.titles.contains(&t) {
                out.titles.push(t);
            }
        }
        queue.extend(line.links.starts.iter().map(String::as_str));
    }
    out
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
    fn the_main_story_completes_while_its_quests_are_cleared() {
        use crate::domain::npc_script::StoryLinks;
        let links = |starts: &[&str], requires: &[&str], gives: &[&str]| StoryLinks {
            starts: starts.iter().map(|s| (*s).to_owned()).collect(),
            requires: requires.iter().map(|s| (*s).to_owned()).collect(),
            gives: gives.iter().map(|s| (*s).to_owned()).collect(),
            titles: vec![7],
            skills: vec!["skill_a".into()],
        };
        let first = links(&["second", "side"], &["q1"], &["q2"]);
        let second = links(&["third"], &["q2"], &[]);
        let third = links(&["fourth"], &["q9"], &[]);
        let fourth = links(&[], &[], &[]);
        let side = links(&[], &[], &[]);
        let line = |id, main, auto, l| StoryFacts {
            id,
            main,
            enabled: true,
            allowed: true,
            auto_give_level: auto,
            reward_title: 0,
            reward_skill: None,
            links: l,
        };
        let lines = [
            line("first", true, 1, &first),
            line("second", true, -1, &second),
            line("third", true, -1, &third),
            line("fourth", true, -1, &fourth),
            line("side", false, -1, &side),
        ];
        let cleared = |q: &str| q == "q1" || q == "q2";
        let plan = story_plan(20, &lines, &cleared);
        assert_eq!(plan.complete, ["first", "second"]);
        // The next main storyline waits for q9; the side one only opens.
        assert_eq!(plan.open, ["side", "third"]);
        assert_eq!(plan.give_quests, ["q2"]);
        assert_eq!(
            (plan.titles, plan.skills),
            (vec![7], vec!["skill_a".to_owned()])
        );
        // Below the root's AutoGiveLevel nothing starts.
        assert_eq!(story_plan(0, &lines, &cleared), StoryPlan::default());
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
