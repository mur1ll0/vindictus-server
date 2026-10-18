# Default-skill multicast

M15.1b.2e.10b.12 implements the ordered GiveSkill multicast suffix and an internal PostgreSQL adapter. It does not implement a CharacterEntity, observable transport, game Create or a successful runtime initializer.

The source SkillManager constructor subscribes its own handler, StatManager.OnSkillRankChanged, then the Skill/MicroPlay observable delegate. After the independent per-skill log, required AP and rank/argument cache mutations precede these runtime calls:

1. For the exact case-sensitive skill_meditation ID, calculate bonus AP time and push AP.
2. ProcessGuildScoreEvent("SkillUp", ID, rank, 1), then PushSkillList(false).
3. Item-skill classification: CalculateItemStat(null,null), CalculateFinalStat, PushUpdateStat. Otherwise stat-skill classification: CalculateFinalStat, PushUpdateStat. Both classifications true select the item branch only.
4. SkillObservable.Updated, then MicroPlayObservable.Updated.

The runtime hook is mandatory and receives the updated cache. Cache/provider failures prevent every runtime call; later failures retain cache and completed effects and stop later subscribers. The dispatcher does not compensate or invent notifications. Source connection/null guards belong inside the actual runtime operation. In particular, CalculateItemStat with no stored EquippedItemInfo and CalculateFinalStat with ItemStat=null must retain their original guards; a projected Gear DTO is not evidence those StatManager fields were loaded.

save_with_ordered_runtime composes this dispatcher with the existing per-log/pre-Submit boundary. Exact database replay runs no callback. A callback failure rolls back uncommitted skill rows while independently committed logs and in-memory effects survive. Outcome.runtime_callbacks_required deliberately remains true for fresh rows: a supplied runtime adapter is not proof that AP persistence, actor state, observers or lifecycle are complete. The storage-only entry point is unchanged.

The new oracle compiles the original manager/stat subscribers, original constructor subscriptions, LoadBaseSkillRank, UpdateSkillArgument, Arg and IsMeditation. Actor operations, AP calculations, stat calculations, classification sets, network pushes and observables are instrumented. Its 256 synthetic cases per Framework x86/x64 cover case sensitivity, overlapping classifications, unchecked rank sums, duplicate/null argument failures and per-operation failure cutoffs. Both architecture results are retained and equal. The tests compare ordered calls and cache state at each call, including attempted failing calls. This is evidence about dispatch, not concrete actor effects. An isolated PostgreSQL test covers success/replay and an observable-hook failure after the first independent log.

## Corrected creation order

The preceding .10b.11 memory incorrectly placed stat/costume callbacks at the first SaveWithoutSync. Original ItemOwner.SaveWithoutSync only invokes Loader.Save, deletion postprocessing and LogManager.Write on success; its failure calls Undo. It never calls Sync or UpdateItemStat. ItemTransactionScope.Dispose does not call Sync either.

Normal ItemImpl.CustomizeCharacter saves defaults, writes ledgers, prepares appearance DTOs, makes its second SaveWithoutSync and returns BaseCostume/ItemCostume/SkinSSSColor. CreateCharacterProcessor loads those into CostumeManager, saves costumes, grants skills and submits Character.Context. SelectCharacterProcessor explicitly queries equipment and initializes item/final stats later. Do not add an earlier stat callback or treat all creation events as already proved no-ops. LoginCharacter, title/status effects and observable subscriptions still need end-to-end closure before certifying fresh detached actor state.

Next: implement the concrete admitted creation runtime (AP and observers with source guards), persist AP/character changes with skills in the same source Submit boundary, and finish title/ItemProxy failure handling. Then LoginLogout, Summary, lifecycle/reconciliation and TCP Create; actual client creation/relogin, selection, city and movement remain required. General equipped/set loaders and complete M01-M24 migration remain in scope. The native lab frontend has not been replaced or retested by this stage.

Readonly private profile aggregate: two initial grants, no Meditation grant, one item-skill callback and no stat-skill callback. This narrows the next implementation to the fresh-actor guards; trained Meditation and the selection equipment loader should not be introduced as Create dependencies without an active source path. The aggregate alone does not prove null stat state, absent observers or unchanged AP.
