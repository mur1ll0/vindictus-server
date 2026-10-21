using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.ItemServiceOperations;
using ServiceCore.CharacterServiceOperations;
class ItemSyncWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v is string || v.GetType().IsPrimitive)return v;
  if(v is IDictionary){var rows=new List<object>();foreach(DictionaryEntry x in (IDictionary)v) rows.Add(new {key=Capture(x.Key),value=Capture(x.Value)});return rows;}
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  var props=v.GetType().GetProperties().Where(p=>p.CanRead).ToArray();
  if(props.Length==0)return v.GetType().GetFields(BindingFlags.Instance|BindingFlags.NonPublic|BindingFlags.Public).ToDictionary(f=>f.Name,f=>Capture(f.GetValue(v)));
  return v.GetType().GetProperties().Where(p=>p.CanRead && p.Name!="Color1" && p.Name!="Color2" && p.Name!="Color3").ToDictionary(p=>p.Name,p=>Capture(p.GetValue(v,null)));
 }
 static T Obj<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetProperty((string)fields[i]).SetValue(obj,fields[i+1],null);
  return obj;
 }
 static SlotInfo Slot(int seed) {
  return Obj<SlotInfo>("ItemID",(long)seed,"ItemClass",seed==0?null:"synthetic_é_装備","Num",seed,"Tab",seed,"Slot",seed,"IsExpireable",seed!=0,"ExpireDateTimeDiff",(long)seed,"ExpireDateTimeTick",long.MaxValue,"Colors",new[]{seed,-1,int.MaxValue},"Durability",seed,"MaxDurability",int.MaxValue,"MaxDurabilityBonus",int.MinValue,"Tradable",true,"ExpanderName","","Attributes",new List<ItemAttributeElement>{null,new ItemAttributeElement("TEST","値",seed,int.MinValue)},"PrefixEnchantStatus",new Dictionary<int,bool>{{-1,true},{7,false}},"SuffixEnchantStatus",new Dictionary<int,bool>(),"MinPrice",int.MinValue,"MaxPrice",int.MaxValue,"AvgPrice",seed,"MinUpTimeDateTimeDiff",long.MinValue);
 }
 static T Priv<T>(string field,object value) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  typeof(T).GetField(field,BindingFlags.Instance|BindingFlags.NonPublic).SetValue(obj,value);
  return obj;
 }
 static CostumeInfo Costume(int seed) {
  var c=new CostumeInfo();
  c.CostumeTypeInfo=new Dictionary<int,int>{{0,seed},{int.MaxValue,-1}};
  c.ColorInfo=new Dictionary<int,int>{{int.MinValue,seed}};
  c.AvatarInfo=new Dictionary<int,bool>{{1,true},{2,false}};
  c.AvatarHideInfo=null;
  c.PollutionInfo=new Dictionary<int,byte>{{3,255}};
  c.EffectInfo=new Dictionary<int,int>();
  c.DecorationInfo=new Dictionary<int,int>{{4,seed}};
  c.DecorationColorInfo=new Dictionary<int,int>{{5,-1}};
  c.Shineness=seed;c.Height=int.MinValue;c.Bust=int.MaxValue;c.PaintingPosX=1;c.PaintingPosY=-1;c.PaintingRotation=seed;c.PaintingSize=2;
  c.HideHeadCostume=seed!=0;c.CafeType=seed;c.IsReturn=seed==0;c.VIPCode=seed;c.BodyPaintingPosX=3;c.BodyPaintingPosY=4;c.BodyPaintingRotation=5;c.BodyPaintingSize=6;c.BodyPaintingSide=7;c.BodyPaintingClip=8;c.BodyPaintingMode=9;
  c.BodyShapeInfo=new Dictionary<int,float>{{0,1.5f},{1,float.NaN},{2,-0.0f}};
  c.TownEffect=seed;
  return c;
 }
    static void AddCases<T>(string kind, int category, Guid expectedGuid, IEnumerable<T> values, List<object> cases) where T : class {
        if (typeof(T).GUID != expectedGuid) {
            throw new InvalidOperationException("GUID mismatch for " + typeof(T).FullName);
        }
        var items = values.ToArray();
        var generic = items.Select(v => SerializeWriter.ToBinary<T>(v)).ToArray();
        ClassInfo<T>.CategoryId = category;
        for (int n = 0; n < items.Length; n++) {
            var typed = SerializeWriter.ToBinary<T>(items[n]);
            foreach (string mode in new[] { "unregistered-generic", "registered-object", "registered-typed" }) {
                var packet = mode == "unregistered-generic" ? generic[n]
                    : mode == "registered-object" ? SerializeWriter.ToBinary((object)items[n])
                    : typed;
                T decoded;
                SerializeReader.FromBinary<T>(packet, out decoded);
                var reencoded = SerializeWriter.ToBinary<T>(decoded);
                if (!Body(reencoded).SequenceEqual(Body(typed))) {
                    throw new InvalidOperationException("Roundtrip mismatch for " + kind + " case " + n);
                }
                // Verify MessageAnalyzer (unencrypted channel transport) frames and deframes packet.Bytes identically.
                var analyzer = new MessageAnalyzer();
                var raw = packet.Bytes;
                var copy = new byte[raw.Count];
                Buffer.BlockCopy(raw.Array, raw.Offset, copy, 0, raw.Count);
                analyzer.Add(new ArraySegment<byte>(copy));
                var analyzed = analyzer.ToArray();
                if (analyzed.Length != 1) {
                    throw new InvalidOperationException("MessageAnalyzer count mismatch");
                }
                var framed = new Packet(analyzed[0]);
                if (framed.CategoryId != packet.CategoryId || !Body(framed).SequenceEqual(Body(packet))) {
                    throw new InvalidOperationException("MessageAnalyzer payload mismatch");
                }
                cases.Add(new {
                    kind,
                    mode,
                    category = packet.CategoryId,
                    body = Body(packet),
                    typed_body = Body(typed),
                    is_null = decoded == null,
                    fields = Capture(decoded)
                });
            }
        }
    }

 static void Main(){
  var cases=new List<object>();
  var updates=new List<UpdateInventoryInfoMessage>{null,Priv<UpdateInventoryInfoMessage>("slotInfos",null),new UpdateInventoryInfoMessage(new List<SlotInfo>()),new UpdateInventoryInfoMessage(new List<SlotInfo>{null}),new UpdateInventoryInfoMessage(new List<SlotInfo>{Obj<SlotInfo>()})};
  foreach(int seed in new[]{0,-1,1,int.MinValue,int.MaxValue})updates.Add(new UpdateInventoryInfoMessage(new List<SlotInfo>{Slot(seed),null,Slot(seed^1)}));
  AddCases("update_inventory_info",617,new Guid("abacd3bc-1a84-3570-a62c-f1b0007675fb"),updates,cases);
  var fails=new List<ItemFailMessage>{null,new ItemFailMessage(null),new ItemFailMessage(Obj<HeroesString>()),new ItemFailMessage(new HeroesString("GameUI_Heroes_EquipItemFail_System")),new ItemFailMessage(new HeroesString("GameUI_Heroes_ConstraintFail_LowLevel",new string[]{null,"","値"})),new ItemFailMessage(new HeroesString(null,new string[0]))};
  AddCases("item_fail",358,new Guid("2f756b90-8fba-38b8-bcb2-a51f0f132502"),fails,cases);
  var costumes=new List<CostumeUpdateMessage>{null,new CostumeUpdateMessage(null),new CostumeUpdateMessage(new CostumeInfo())};
  foreach(int seed in new[]{0,-1,1,int.MinValue,int.MaxValue})costumes.Add(new CostumeUpdateMessage(Costume(seed)));
  var moves=new List<MoveInventoryItemMessage>{null};
  foreach(long id in new[]{long.MinValue,-1L,0L,long.MaxValue})foreach(byte st in new byte[]{0,6,100,255})foreach(int sl in new[]{int.MinValue,-1,0,47,48,int.MaxValue}){var m=new MoveInventoryItemMessage();m.ItemID=id;m.Storage=st;m.Target=sl;moves.Add(m);}
  AddCases("move_inventory_item",502,new Guid("7c957f24-e8d7-338f-a93c-97bc6903f320"),moves,cases);
  var bundles=new List<EquipBundleMessage>{null};
  foreach(string c in new[]{null,"","life_potion_lvl0","skill_synthetic"})foreach(int q in new[]{int.MinValue,-1,0,3,100,119,int.MaxValue})bundles.Add(new EquipBundleMessage(c,q));
  AddCases("equip_bundle",456,new Guid("6652c42f-bf01-3da0-9415-f390b085e456"),bundles,cases);
  var quick=new List<QuickSlotInfoMessage>{null,new QuickSlotInfoMessage(null),new QuickSlotInfoMessage(new QuickSlotInfo(null)),new QuickSlotInfoMessage(new QuickSlotInfo(new Dictionary<int,string>())),new QuickSlotInfoMessage(new QuickSlotInfo(new Dictionary<int,string>{{3,""},{100,"life_potion_lvl0"},{101,null},{int.MinValue,"synthetic"}}))};
  AddCases("quick_slot_info",551,new Guid("edc4290e-196c-3f17-903f-d0f3317e47d9"),quick,cases);
  AddCases("query_ap",526,new Guid("f061ae98-f5c8-333d-82ca-3b217e268120"),new[]{null,new QueryAPMessage()},cases);
  var aps=new List<APMessage>{null};
  foreach(int a in new[]{int.MinValue,-1,0,500,int.MaxValue})foreach(long t in new[]{long.MinValue,-1L,0L,long.MaxValue})aps.Add(new APMessage(a,a==0?-1:1000,t,a==500?-1:60));
  AddCases("ap",229,new Guid("3d59ac68-6b3c-3f92-968e-b569b973f065"),aps,cases);
  var uses=new List<UseInventoryItemMessage>{null};
  foreach(long id in new[]{long.MinValue,-1L,0L,long.MaxValue})foreach(long tid in new[]{-1L,0L,long.MaxValue})uses.Add(new UseInventoryItemMessage(id,tid));
  AddCases("use_inventory_item",629,new Guid("c182cb18-7469-3390-a0e7-8eba31d6e45d"),uses,cases);
  AddCases("use_inventory_item_ok",627,new Guid("451d459f-bb17-325c-bb10-d608f03f4c3f"),new[]{null,Obj<UseInventoryItemOkMessage>(),Obj<UseInventoryItemOkMessage>("UseItemClass",""),Obj<UseInventoryItemOkMessage>("UseItemClass","capsule_ap50")},cases);
  AddCases("query_skill_list",540,new Guid("860e6484-d1b5-34a9-9412-70807bbbf79c"),new[]{null,new QuerySkillListMessage()},cases);
  var lists=new List<SkillListMessage>{null,new SkillListMessage(null,0,null,false),new SkillListMessage("",0,new List<BriefSkillInfo>(),true)};
  foreach(int seed in new[]{int.MinValue,-1,0,1,30,int.MaxValue}){
   var b=new BriefSkillInfo{SkillID=seed==0?null:"skill_synthetic",BaseRank=seed,FinalRank=seed,RequiredAP=-1,IsLocked=(byte)(seed&255),CanStartTraining=(byte)(seed==30?1:0),CurrentAP=seed,ResetSkillAP=-1,UntrainSkillAP=seed};
   if(seed==1)b.Enhances=null;
   if(seed==30)b.Enhances.Add(new BriefSkillEnhance{GroupKey=seed,IndexKey=-1,Type=255,ReduceDurability=int.MinValue,MaxDurabilityBonus=int.MaxValue});
   lists.Add(new SkillListMessage(seed==-1?null:"skill_learning",seed,new List<BriefSkillInfo>{b,null},seed%2==0));
  }
  AddCases("skill_list",583,new Guid("dd3b94a9-64da-381a-b2e7-0fad58835746"),lists,cases);
  var learns=new List<LearnSkillMessage>{null};
  foreach(string sk in new[]{null,"","skill_fiona_guard"})foreach(int ap in new[]{int.MinValue,-1,0,30,int.MaxValue})learns.Add(new LearnSkillMessage(sk,ap));
  AddCases("learn_skill",475,new Guid("3ec00564-95e3-3507-b9dc-a5a185215b19"),learns,cases);
  var completions=new List<SkillCompletionMessage>{null};
  foreach(string sk in new[]{null,"skill_fiona_guard"})foreach(int r in new[]{int.MinValue,0,2,int.MaxValue})completions.Add(new SkillCompletionMessage(sk,r));
  AddCases("skill_completion",582,new Guid("2b5614fc-8830-3aec-a03f-c33c270de9f1"),completions,cases);
  var commons=new List<QueryCharacterCommonInfoMessage>{null};
  foreach(long q in new[]{long.MinValue,0L,7L,long.MaxValue})foreach(long c in new[]{-1L,1L,long.MaxValue})commons.Add(Obj<QueryCharacterCommonInfoMessage>("QueryID",q,"CID",c));
  AddCases("query_common_info",527,new Guid("c9cda3f6-8b67-3c69-ba31-ba2fe827fd7b"),commons,cases);
  var news=new List<LearnNewSkillMessage>{null};
  foreach(string sk in new[]{null,"","skill_fiona_guard"})news.Add(new LearnNewSkillMessage(sk));
  AddCases("learn_new_skill",340,new Guid("7fbd47e1-96a3-3472-9e71-f36d90f7294f"),news,cases);
  var results=new List<LearnNewSkillResultMessage>{null};
  foreach(LearnNewSkillResultMessage.LearnNewSkillResult r in Enum.GetValues(typeof(LearnNewSkillResultMessage.LearnNewSkillResult)))results.Add(new LearnNewSkillResultMessage(r));
  AddCases("learn_new_skill_result",341,new Guid("0430910f-e20e-3736-850d-b619e836f14f"),results,cases);
  AddCases("query_npc_list",533,new Guid("6d029dfe-89e5-32d0-aa79-27d298e3b87c"),new[]{null,new QueryNpcListMessage()},cases);
  var stories=new List<StoryInfo>{new StoryInfo(null,false),null,new StoryInfo("story_synthetic",true)};
  var npcs=new List<NpcInfo>{new NpcInfo(null,null,null),null,new NpcInfo("FERGHUS","",new List<StoryInfo>()),new NpcInfo("ARIEL","S2EP1&&SpiritInjection",stories)};
  var buildings=new List<BuildingInfo>{new BuildingInfo(null,null),null,new BuildingInfo("t01_bs",npcs),new BuildingInfo("t01_inn",new List<NpcInfo>())};
  AddCases("npc_list",507,new Guid("35be098a-89fe-3185-a8f3-1cf91f5c07c1"),new[]{null,new NpcListMessage(null),new NpcListMessage(new List<BuildingInfo>()),new NpcListMessage(buildings)},cases);
  var talks=new List<QueryNpcTalkMessage>{null};
  foreach(string loc in new[]{null,"","t01_bs"})foreach(string npc in new[]{null,"FERGHUS"})foreach(string sl in new[]{null,"default"})foreach(string cmd in new[]{null,"greeting"})talks.Add(new QueryNpcTalkMessage(loc,npc,sl,cmd));
  AddCases("query_npc_talk",534,new Guid("986c6c40-c9b6-3aed-9303-0b5679aa8f53"),talks,cases);
  var entities=new List<NpcTalkEntity>{new NpcTalkEntity("talk","HEROES_STORYLINE_SYNTHETIC"),null,new NpcTalkEntity("change_npc",null,"x"),Obj<NpcTalkEntity>("Command",null,"Arg1",null,"Arg2",null)};
  AddCases("npc_talk",508,new Guid("859ce7b4-cfd1-3695-9a73-b5c094d7085e"),new[]{null,new NpcTalkMessage(null),new NpcTalkMessage(new List<NpcTalkEntity>()),new NpcTalkMessage(entities)},cases);
  var buttons=new List<SelectButtonMessage>{null};
  foreach(int b in new[]{int.MinValue,-1,0,3,int.MaxValue})buttons.Add(new SelectButtonMessage(b));
  AddCases("select_button",564,new Guid("d5dda62d-44c1-3c6a-8906-72164e564921"),buttons,cases);
  AddCases("execute_npc_server_command",459,new Guid("048fdbb2-e452-3c5f-b0b2-cfc80994e365"),new[]{null,new ExecuteNpcServerCommandMessage()},cases);
  var briefs=new List<BriefStoryLineInfo>{new BriefStoryLineInfo(null,null,255),null,new BriefStoryLineInfo("GuidePlay_1_SkillTrain","p0",1,"goal_text"),new BriefStoryLineInfo("S0NT_99_CutScene","",0,null)};
  AddCases("story_lines",598,new Guid("5498b052-d119-30e2-8224-1e2b990d4ef8"),new[]{null,new StoryLinesMessage(null,0),new StoryLinesMessage(new List<BriefStoryLineInfo>(),int.MinValue),new StoryLinesMessage(briefs,3)},cases);
  var buys=new List<BuyItemMessage>{null};
  foreach(string sh in new[]{null,"","FERGHUS_Shop"})foreach(int o in new[]{-1,0,7})foreach(int c in new[]{int.MinValue,0,1,99})buys.Add(Obj<BuyItemMessage>("ShopID",sh,"Order",o,"Count",c));
  AddCases("buy_item",234,new Guid("2f52e151-33c1-34dc-bd6e-9535c35ab918"),buys,cases);
  AddCases("buy_item_result",235,new Guid("64a08bdf-6762-3be2-9f6f-c7ff64a2b4a3"),new[]{null,new BuyItemResultMessage(BuyItemResultMessage.BuyItemResult.Failed,null,0,null,0),new BuyItemResultMessage(BuyItemResultMessage.BuyItemResult.Succeeded,"potion_hp_lvl1",10,"gold",500,3,-1),new BuyItemResultMessage(BuyItemResultMessage.BuyItemResult.Failed_UniqueConstraint,"x",int.MaxValue,"",int.MinValue,int.MinValue,int.MaxValue)},cases);
  AddCases("query_shop_item_info",167,new Guid("4088bd54-bc81-3398-8386-61bdaf5917c1"),new[]{null,Obj<QueryShopItemInfoMessage>("ShopID",null),Obj<QueryShopItemInfoMessage>("ShopID",""),Obj<QueryShopItemInfoMessage>("ShopID","FERGHUS_Shop")},cases);
  var sells=new List<SellItemMessage>{null};
  foreach(long id in new[]{long.MinValue,-1L,0L,long.MaxValue})foreach(int c in new[]{int.MinValue,0,1,int.MaxValue})sells.Add(Obj<SellItemMessage>("ItemID",id,"Count",c));
  AddCases("sell_item",566,new Guid("62669042-85a8-354c-9cab-ccae3c2becae"),sells,cases);
  AddCases("query_buyback_list",293,new Guid("2271100a-effc-3693-b75d-165507d49b79"),new[]{null,new QueryBuybackListMessage()},cases);
  var backs=new List<BuybackInfo>{new BuybackInfo(0,"potion",3,-1,-1,-1,40),null,new BuybackInfo(int.MaxValue,null,int.MinValue,1,2,3,int.MaxValue)};
  backs[2].ShopID=null;backs[2].PriceItemClass=null;
  AddCases("buyback_list_result",294,new Guid("88ce410b-fcb7-3794-ab5f-9cb25771ce3d"),new[]{null,new BuybackListResultMessage(null),new BuybackListResultMessage(new List<BuybackInfo>()),new BuybackListResultMessage(backs)},cases);
  var repairs=new List<RepairItemMessage>{null,new RepairItemMessage(null),new RepairItemMessage(new List<long>{long.MinValue,0L,long.MaxValue}){AddAllEquippedItems=true,AddAllRepairableItems=true}};
  var nullIds=new RepairItemMessage(new List<long>());nullIds.ItemIDs=null;nullIds.AddAllBrokenItems=true;repairs.Add(nullIds);
  AddCases("repair_item",424,new Guid("b83c8562-c97c-343c-b421-d3cdada26774"),repairs,cases);
  AddCases("update_story_guide",622,new Guid("4f661234-1161-3db9-8127-877ed7807435"),new[]{null,new UpdateStoryGuideMessage(null,null),new UpdateStoryGuideMessage("",""),new UpdateStoryGuideMessage("colhen.adv_guild","HEROES_STORYLINE_GUIDE")},cases);
  var vars=new List<StoryVariableInfo>{new StoryVariableInfo("GuidePlay","count",3),null,new StoryVariableInfo(null,null,int.MinValue)};
  AddCases("story_variables",599,new Guid("de76b127-f09d-369a-8008-26af3e5c8383"),new[]{null,new StoryVariablesMessage(null),new StoryVariablesMessage(new List<StoryVariableInfo>()),new StoryVariablesMessage(vars)},cases);
  AddCases("query_quest_progress",535,new Guid("5b634b00-c047-339e-a4fe-b0ef8e3b9a2e"),new[]{null,new QueryQuestProgressMessage()},cases);
  var times=new List<ClearTimeElement>{new ClearTimeElement{Difficulty=2,MemberCount=4,ClearTime=612,PlayRank=1,ClearInfo=7,IsOld=true},new ClearTimeElement()};
  var goal=new AchieveGoalInfo(10010,2,3,255,new DateTime(2020,1,1));goal.LastClearTimeDiff=int.MinValue;
  var progress=new List<QuestProgressInfo>{new QuestProgressInfo("quest_gnoll_watchman",0,0,true,true,false,true,-1,0,new List<ClearTimeElement>()),null,new QuestProgressInfo(null,int.MinValue,int.MaxValue,false,false,true,false,5,4,null),new QuestProgressInfo("quest_x",100,2,false,true,true,true,int.MaxValue,1,times)};
  AddCases("quest_progress",546,new Guid("c229a0f5-4eda-3bbc-a4be-61f3aad3f82b"),new[]{null,new QuestProgressMessage(null,null),new QuestProgressMessage(new List<QuestProgressInfo>(),new List<AchieveGoalInfo>()),new QuestProgressMessage(new List<QuestProgressInfo>{progress[0],null},null),new QuestProgressMessage(new List<QuestProgressInfo>{progress[2]},null),new QuestProgressMessage(new List<QuestProgressInfo>{progress[3]},null),new QuestProgressMessage(null,new List<AchieveGoalInfo>{goal}),new QuestProgressMessage(null,new List<AchieveGoalInfo>{null})},cases);
  AddCases("costume_update",447,new Guid("099b778f-141e-359d-b270-60c9d2f99bbb"),costumes,cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Utility { public static class Log<T> { public static readonly DummyLogger Logger=new DummyLogger(); } public class DummyLogger {public void DebugFormat(string f,params object[] a){} } }
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
