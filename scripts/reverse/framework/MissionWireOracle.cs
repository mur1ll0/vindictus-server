using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Net;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Utility;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.CharacterServiceOperations;
using ServiceCore.MicroPlayServiceOperations;
using ServiceCore.EndPointNetwork.MicroPlay;
using ServiceCore.ItemServiceOperations;
class MissionWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static T Obj<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetProperty((string)fields[i]).SetValue(obj,fields[i+1],null);
  return obj;
 }
 static T Priv<T>(string field,object value) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  typeof(T).GetField(field,BindingFlags.Instance|BindingFlags.NonPublic).SetValue(obj,value);
  return obj;
 }
 static void AddCases<T>(string kind,int category,Guid expectedGuid,IEnumerable<T> values,List<object> cases) where T:class {
  if(typeof(T).GUID!=expectedGuid)throw new InvalidOperationException("GUID mismatch for "+typeof(T).FullName);
  ClassInfo<T>.CategoryId=category;
  int n=0;
  foreach(var v in values){
   var typed=SerializeWriter.ToBinary<T>(v);
   T decoded;
   SerializeReader.FromBinary<T>(typed,out decoded);
   if(!Body(SerializeWriter.ToBinary<T>(decoded)).SequenceEqual(Body(typed)))throw new InvalidOperationException("Roundtrip mismatch for "+kind+" case "+n);
   cases.Add(new{kind,category=typed.CategoryId,body=Body(typed),is_null=v==null});
   n++;
  }
 }
 static ShipOptionInfo Option(int seed){
  var o=new ShipOptionInfo(seed,4,-1,1,int.MaxValue,0,seed&1,seed==0?0:2);
  o.IsSeason2=seed!=0;o.SelectedBossQuestIDInfos=seed==0?null:(ICollection<int>)new List<int>{seed,int.MinValue};o.IsPracticeMode=seed==1;o.UserDSMode=false;
  return o;
 }
 static PartyMemberInfo Member(int seed){
  return Obj<PartyMemberInfo>("NexonSN",seed,"Character",(BaseCharacter)1,"CharacterID",seed==0?null:"Fiona_é","SlotNum",seed&3,"Level",seed,"State",(ReadyState)3,"LatestPing",-1,"LatestFrameRate",int.MaxValue,"IsReturn",seed==1,"IsEventJumping",true);
 }
 static ShipInfo Ship(int seed){
  return Obj<ShipInfo>("PartyID",(long)seed<<33,"ShipName",seed==0?null:"","Password",null,"MinLevelConstraint",1,"MaxLevelConstraint",-1,"MemberCount",1,"MaxShipMemberCount",4,"QuestID","quest_gnoll_scout","IsHuntingQuest",false,"IsGiantRaid",seed==2,"SwearID",-1,"RestTime",int.MinValue,"ReinforceAllowed",true,"AdultRule",false,"Members",seed==0?null:new List<PartyMemberInfo>{Member(seed),null},"State",(PartyInfoState)1,"PartyBonusCount",0,"PartyBonusRatio",seed,"IsPartyOnly",true,"HostPing",3,"HostFrameRate",60,"Difficulty",seed,"IsReturn",false,"IsSeason2",true,"selectedBossQuestIDInfos",seed==0?null:(ICollection<int>)new List<int>{7},"IsPracticeMode",false,"UserDSMode",seed==1);
 }
 static CostumeInfo Costume(int seed){
  var c=new CostumeInfo();
  c.CostumeTypeInfo=new Dictionary<int,int>{{0,seed}};c.ColorInfo=new Dictionary<int,int>();c.AvatarInfo=new Dictionary<int,bool>{{1,true}};c.AvatarHideInfo=null;
  c.PollutionInfo=new Dictionary<int,byte>();c.EffectInfo=new Dictionary<int,int>();c.DecorationInfo=new Dictionary<int,int>();c.DecorationColorInfo=new Dictionary<int,int>();
  c.Height=seed;c.Bust=-1;c.BodyShapeInfo=new Dictionary<int,float>{{0,1.5f}};c.TownEffect=seed;
  return c;
 }
 static GameJoinMemberInfo Join(int seed){
  var g=new GameJoinMemberInfo();
  g.Order=seed;g.Tag=seed;g.Key=-1;g.Name=seed==0?null:"Fiona_é";g.BaseClass=1;g.Level=seed;g.Exp=int.MaxValue;g.LevelUpExp=int.MinValue;g.TitleID=0;g.TitleCount=2;
  if(seed==0){g.Stats=null;g.BattleInventory=null;g.CostumeInfo=null;g.SkillList=null;g.SpSkills=null;g.VocationSkills=null;g.SkillEnhanceDic=null;g.DefMap=null;g.ArmorHPMap=null;g.EquippedItems=null;g.AbilityList=null;g.StatusEffectDict=null;g.StatusEffects=null;g.GuildLevelBonus=null;}
  else{
   g.Stats=new Dictionary<string,int>{{"STR",seed},{"ATK",-1}};
   var bi=new BattleInventory();bi.Consumables.Add(3,new ConsumablesInfo("potion_hp_lvl1",5,2));if(seed==2){var box=new ConsumablesInfo("potion_box",1,0);box.InnerConsumables.Add(new ConsumablesInfo("potion_inner",3,1));bi.Consumables.Add(5,box);}bi.Consumables.Add(4,null);g.BattleInventory=bi;
   g.CostumeInfo=Costume(seed);
   g.SkillList=new Dictionary<string,int>{{"skill_fiona_guard",1}};
   g.SpSkills=new Dictionary<int,string>{{1,"sp"},{2,null}};
   g.VocationSkills=new Dictionary<string,int>();
   g.SkillEnhanceDic=new Dictionary<string,BriefSkillEnhance>{{"skill_x",new BriefSkillEnhance{GroupKey=1,IndexKey=2,Type=3,ReduceDurability=4,MaxDurabilityBonus=5}},{"skill_y",null}};
   g.DefMap=new Dictionary<int,int>{{0,10}};g.ArmorHPMap=new Dictionary<int,int>{{1,int.MaxValue}};
   g.EquippedItems=new Dictionary<int,string>{{0,"sword"}};
   g.AbilityList=new List<int>{1,-1};
   g.StatusEffectDict=new Dictionary<string,int>{{"buff",3}};
   g.StatusEffects=new List<StatusEffectElement>{new StatusEffectElement("buff",1,-1,0)};
   g.GuildLevelBonus=new Dictionary<string,int>();
  }
  g.Pet=null;g.TransformCoolDown=-1;g.DestroyedDef=seed;g.IsTeacher=false;g.IsAssist=seed==2;g.IsAlive=true;
  return g;
 }
 static QuestSectorInfos Sectors(int seed){
  var s=new QuestSectorInfos();
  s.CurrentGroup=seed;s.StartingGroup=1;s.TreasureBoxMaxCount=-1;
  if(seed==0){s.AltarStatus=null;s.BossKilledList=null;typeof(QuestSectorInfos).GetProperty("SectorInfos").SetValue(s,null,null);return s;}
  var con=new List<QuestSectorConnectionInfo>{Obj<QuestSectorConnectionInfo>("From",1,"FromTrigger","exit_a","To",2,"ToSpawn","")};
  var a=new QuestSectorInfo(1,"01h",new List<string>{"gnoll_a",null},con,true,null);
  a.ItemDropEntity.Add("gnoll_a",2);a.WeakPointEntity.Add("ogre",-1);
  a.TrialFactorInfos.Add(Obj<TrialFactorInfo>("GroupNumber",1,"TrialMod",2,"TrialName","trial","SectorGroupID",1));
  a.TreasureBoxInfos.Add(Obj<TreasureBoxInfo>("GroupID",1,"EntityName","box","IsVisible",true,"IsOpend",false));
  s.SectorInfos.Add(a);
  s.SectorInfos.Add(Obj<QuestSectorInfo>("GroupID",2,"Bsp",null,"Entities",null,"ConnectionInfos",null,"ItemDropEntity",null,"WeakPointEntity",null,"TrialFactorInfos",null,"TreasureBoxInfos",null,"SaveRequired",false,"LastUsedSpawnPoint","spawn"));
  s.AltarStatus.Add(3);s.BossKilledList.Add("boss");s.BossKilledList.Add(null);
  return s;
 }
 // The recovered reader repeats the previous instance for a null element of a class list
 // (List<T> of reference types); the server never sends such elements, so cases avoid them.
 static void Main(){
  var cases=new List<object>();
  var accepts=new List<AcceptQuestMessage>{null,new AcceptQuestMessage(null,null,0,null,false,null)};
  foreach(int seed in new[]{0,1,int.MaxValue})accepts.Add(new AcceptQuestMessage(seed==0?"":"quest_gnoll_scout",seed==1?"title_é":null,seed,Option(seed),false,null));
  AddCases("accept_quest",224,new Guid("aecbcd0a-82b3-31ee-b771-fe9c53dabdd8"),accepts,cases);
  var parties=new List<PartyInfoMessage>{null,Obj<PartyInfoMessage>("PartyID",0L,"PartySize",0,"State",(PartyInfoState)0,"Members",null)};
  foreach(int seed in new[]{1,5})parties.Add(Obj<PartyInfoMessage>("PartyID",long.MaxValue-seed,"PartySize",4,"State",(PartyInfoState)seed,"Members",new List<PartyMemberInfo>{Member(seed),null,Member(0)}));
  AddCases("party_info",511,new Guid("78d2aa77-13f9-302f-b9fe-67ca096b92f3"),parties,cases);
  AddCases("update_ship",618,new Guid("c87e7f92-c8c2-36b1-83e8-bc9fbd04e5d4"),new[]{null,new UpdateShipMessage(null),new UpdateShipMessage(Ship(0)),new UpdateShipMessage(Ship(1)),new UpdateShipMessage(Ship(2))},cases);
  var requests=new List<ConnectionRequestMessage>{null,Obj<ConnectionRequestMessage>("Address",null,"Port",(ushort)0,"PosixTime",0,"Key",0,"Category",null,"PingHostCID",0L,"GroupID",0L)};
  foreach(int sn in new[]{1,int.MaxValue,-5})requests.Add(new ConnectionRequestMessage(new IPEndPoint(IPAddress.Parse("127.0.0.1"),sn==1?27015:65535),sn.ToDateTime(),sn^7,"MicroPlayServiceCore.MicroPlayService"));
  AddCases("connection_request",425,new Guid("31fc12c3-d398-355f-a15c-786e69ccc95f"),requests,cases);
  AddCases("launch_ship_granted",473,new Guid("78df701a-83a3-33f5-be94-8d27ecb6e23b"),new[]{null,new LaunchShipGrantedMessage(null,0,0,null),new LaunchShipGrantedMessage("quest_gnoll_scout",1,255,Join(0)),new LaunchShipGrantedMessage("",0,1,Join(1)),new LaunchShipGrantedMessage("q",0,0,Join(2))},cases);
  var infos=new[]{null,Obj<GameInfo>("Name",null,"Map",null,"GameDir",null,"Description",null,"HostID",0,"DSIP",null,"DSPort",0),Obj<GameInfo>("Name","Fiona_é","Map","01h","GameDir","heroes","Description","","HostID",-1,"DSIP","10.0.0.1","DSPort",int.MaxValue)};
  AddCases("register_server",554,new Guid("b8a9e8c0-377b-35ad-b3be-bb911152a565"),new[]{null,new RegisterServerMessage(infos[0]),new RegisterServerMessage(infos[1]),new RegisterServerMessage(infos[2])},cases);
  AddCases("start_game",591,new Guid("7c04c215-b83c-3053-b80c-966064258660"),new[]{null,new StartGameMessage()},cases);
  var grants=new List<GameStartGrantedMessage>{null};
  foreach(int seed in new[]{0,1,-1})grants.Add(Obj<GameStartGrantedMessage>("QuestLevel",seed,"QuestTime",seed*600,"HardMode",seed&1,"SoloSector",-1,"QuestSectorInfos",seed==-1?null:Sectors(seed),"IsHuntingQuest",seed==1,"InitGameTime",0,"SectorMoveGameTime",int.MaxValue,"Difficulty",2,"IsTimerDecreasing",true,"QuestStartedPlayerCount",1,"NewSlot",-1,"NewKey",-1,"IsUserDedicated",false));
  AddCases("game_start_granted",462,new Guid("dc3acd0b-6946-3b6c-bcaf-29057e0dfc59"),grants,cases);
  AddCases("member_info",498,new Guid("b89c49cb-0aa1-308d-a0c5-9fe9bb1f58ed"),new[]{null,new MemberInfoMessage(0,null),new MemberInfoMessage(3,Join(1))},cases);
  var coins=new List<PayCoinCompletedMessage>{null,new PayCoinCompletedMessage(null),new PayCoinCompletedMessage(new List<PaidCoinInfo>())};
  coins.Add(new PayCoinCompletedMessage(new List<PaidCoinInfo>{Obj<PaidCoinInfo>("Slot",0,"SilverCoin",(ICollection<int>)new List<int>(),"PlatinumCoinOwner",-1,"PlatinumCoinType",CoinType.First),Obj<PaidCoinInfo>("Slot",3,"SilverCoin",null,"PlatinumCoinOwner",2,"PlatinumCoinType",CoinType.FatiguePoint)}));
  AddCases("pay_coin_completed",432,new Guid("7f105751-b6a7-3c1d-a7fd-bc7a884133d2"),coins,cases);
  var stones=new List<InsertBlessStoneCompleteMessage>{null,Obj<InsertBlessStoneCompleteMessage>("Slot",0,"OwnerList",null,"TypeList",null)};
  var stone=new InsertBlessStoneCompleteMessage{Slot=0};foreach(var t in new[]{BlessStoneType.ALL,BlessStoneType.AP,BlessStoneType.EXP,BlessStoneType.LUCK})stone.AddInfo(t,-1);stones.Add(stone);
  var mixed=new InsertBlessStoneCompleteMessage{Slot=int.MaxValue};mixed.AddInfo(BlessStoneType.NONE,3);stones.Add(mixed);
  AddCases("insert_bless_stone_complete",48,new Guid("915f8913-1565-340c-99ca-619e37ffd1f7"),stones,cases);
  var moves=new List<MoveToNextSectorMessage>{null,Obj<MoveToNextSectorMessage>("TriggerName",null,"TargetGroup",0,"HolyProps",null)};
  foreach(int g in new[]{-1,1,int.MaxValue})moves.Add(Obj<MoveToNextSectorMessage>("TriggerName",g==1?"":"exit_é","TargetGroup",g,"HolyProps",new List<int>{g,0}));
  AddCases("move_to_next_sector",532,new Guid("67aefac8-659d-385a-a39a-03e3d1dce102"),moves,cases);
  AddCases("all_user_join_complete",47,new Guid("3f79e7b8-49d5-34f2-8970-b8b5b47aa1eb"),new[]{null,new AllUserJoinCompleteMessage()},cases);
  var kills=new List<MonsterKilledMessage>{null,Obj<MonsterKilledMessage>("Attacker",0,"Target",0,"ActionType",null,"HasEvilCore",false,"Damage",0,"DamagePositionX",0,"DamagePositionY",0,"Distance",0,"ActorIndex",0)};
  foreach(int seed in new[]{-1,1,int.MaxValue})kills.Add(Obj<MonsterKilledMessage>("Attacker",seed,"Target",seed^256,"ActionType",seed==1?"no_item":"attack_é","HasEvilCore",seed==1,"Damage",seed,"DamagePositionX",-seed,"DamagePositionY",seed,"Distance",int.MinValue,"ActorIndex",seed));
  AddCases("monster_killed",501,new Guid("8cfddc5b-72e0-3490-bb44-1ecf1a813468"),kills,cases);
  AddCases("player_killed",522,new Guid("71d3965f-66a2-31f1-aa15-90d54c7c7882"),new[]{null,Priv<PlayerKilledMessage>("tag",0),Priv<PlayerKilledMessage>("tag",int.MinValue),Priv<PlayerKilledMessage>("tag",3)},cases);
  var kicks=new List<RagdollKickedMessage>{null,Obj<RagdollKickedMessage>("Tag",0,"TargetEntityName",null,"EvilCoreType",EvilCoreType.None,"IsRareCore",false)};
  foreach(int seed in new[]{0,3,int.MaxValue})kicks.Add(Obj<RagdollKickedMessage>("Tag",seed,"TargetEntityName",seed==0?"":"257/Target","EvilCoreType",(EvilCoreType)(seed&7),"IsRareCore",seed==3));
  AddCases("ragdoll_kicked",552,new Guid("e7ae9d95-805f-3478-b19e-b03c14234ee7"),kicks,cases);
  AddCases("drop_gold_core",350,new Guid("d01fb1ee-5a73-3dee-834a-84687c126f14"),new[]{null,new DropGoldCoreMessage(null,null,0),new DropGoldCoreMessage("257",new List<EvilCoreInfo>{new EvilCoreInfo("257",EvilCoreType.BasicGoldCore,0,true),new EvilCoreInfo("box",EvilCoreType.AdditionalCore,-1)},1),new DropGoldCoreMessage("",new List<EvilCoreInfo>(),int.MinValue)},cases);
  AddCases("get_item",463,new Guid("cd70d18d-dc3b-3053-b9fe-6efbc76d28bb"),new[]{null,new GetItemMessage(null,null,0,0,false,0,GiveItem.ResultEnum.Success),new GetItemMessage("Fiona_é","cloth_lvl1",1,1,false,0,GiveItem.ResultEnum.Success),new GetItemMessage("x","gold",int.MaxValue,10,true,-1,GiveItem.ResultEnum.Failed)},cases);
  AddCases("sector_prop_list",563,new Guid("64e27db4-48c9-3507-a366-9bbb6b1b6a79"),new[]{null,Obj<SectorPropListMessage>("Props",null),Obj<SectorPropListMessage>("Props",(IDictionary<int,int>)new Dictionary<int,int>()),Obj<SectorPropListMessage>("Props",(IDictionary<int,int>)new Dictionary<int,int>{{12,3},{-1,int.MaxValue},{7,3}})},cases);
  AddCases("prop_broken",524,new Guid("15af6960-d57a-3be7-820c-d8825a163a0b"),new[]{null,new PropBrokenMessage(0,null,0),new PropBrokenMessage(12,"barrel",-1),new PropBrokenMessage(int.MaxValue,"",3)},cases);
  var gold=new ErgInfo(3,"gold","gold",40);gold.Winner=0;
  AddCases("drop_erg",455,new Guid("64b4f44b-be70-3e40-a441-a61ceaf133ea"),new[]{null,new DropErgMessage(0,null),new DropErgMessage(12,new List<ErgInfo>{gold,new ErgInfo("INSTANT","HP",65)}),new DropErgMessage("257",new List<ErgInfo>{new ErgInfo(-1,"server","life_erg_crystal",int.MaxValue)})},cases);
  AddCases("pick_erg",520,new Guid("2e9ee02b-6439-3079-9834-932286dfbb4b"),new[]{null,new PickErgMessage(0),new PickErgMessage(-1),new PickErgMessage(int.MaxValue)},cases);
  AddCases("put_erg",525,new Guid("d7f2e4f9-11e7-369c-a4dd-70d46756e0ad"),new[]{null,new PutErgMessage(0,0),new PutErgMessage(3,1),new PutErgMessage(int.MinValue,-1)},cases);
  var hits=new List<MonsterTakeDamageInfo>{Obj<MonsterTakeDamageInfo>("Attacker",0,"AttackTime",1.5f,"ActionName","attack_é","Damage",120),Obj<MonsterTakeDamageInfo>("Attacker",-1,"AttackTime",float.NaN,"ActionName",null,"Damage",int.MinValue)};
  AddCases("monster_damage_report",290,new Guid("62bd766f-9899-36d4-a6fc-0e4e448471c2"),new[]{null,Obj<MonsterDamageReportMessage>("Target",0,"TakeDamageList",null),Obj<MonsterDamageReportMessage>("Target",257,"TakeDamageList",(ICollection<MonsterTakeDamageInfo>)hits),Obj<MonsterDamageReportMessage>("Target",-1,"TakeDamageList",(ICollection<MonsterTakeDamageInfo>)new List<MonsterTakeDamageInfo>())},cases);
  AddCases("combat_record",263,new Guid("1c979684-101a-325c-bf59-caca716604fb"),new[]{null,Obj<CombatRecordMessage>("PlayerNumber",0,"ComboMax",0,"HitMax",0,"StyleMax",0,"Death",0,"Kill",0,"BattleAchieve",0,"HitTake",0,"StyleCount",0,"RankStyle",0,"RankBattle",0,"RankTotal",0),Obj<CombatRecordMessage>("PlayerNumber",1,"ComboMax",2,"HitMax",3,"StyleMax",4,"Death",5,"Kill",6,"BattleAchieve",7,"HitTake",8,"StyleCount",9,"RankStyle",10,"RankBattle",-1,"RankTotal",int.MaxValue)},cases);
  AddCases("use_consumable",624,new Guid("8cf9f82f-0433-31a7-b88d-d61b71ee61ee"),new[]{null,new UseConsumableMessage(0,100,0),new UseConsumableMessage(-1,int.MaxValue,int.MinValue)},cases);
  AddCases("player_revived",555,new Guid("0441624d-0d61-3f4d-8194-090df4655d18"),new[]{null,Obj<PlayerRevivedMessage>("CasterTag",0,"ReviverTag",0,"Method",null),Obj<PlayerRevivedMessage>("CasterTag",1,"ReviverTag",0,"Method","phoenix_feather")},cases);
  AddCases("apply_micro_play_effects",403,new Guid("10c709a9-28c3-328e-8dfa-6bb6f5ecc0c8"),new[]{null,Obj<ApplyMicroPlayEffectsMessage>("Caster",0,"EffectName",null,"EffectList",null),Obj<ApplyMicroPlayEffectsMessage>("Caster",1,"EffectName","phoenix_feather","EffectList",new List<MicroPlayEffect>{Obj<MicroPlayEffect>("PlayerSlotNo",0,"Effect",MicroPlayEffectType.ReviveByClient,"Amount",50,"Argument",null),Obj<MicroPlayEffect>("PlayerSlotNo",0,"Effect",MicroPlayEffectType.GetStatusByClient,"Amount",1,"Argument","revive_invincible_normal")})},cases);
  AddCases("micro_play_event",362,new Guid("52a3dbeb-8337-3288-9005-5557bfa8e7e0"),new[]{null,new MicroPlayEventMessage(0,null),new MicroPlayEventMessage(-1,"kill boss_é attack")},cases);
  AddCases("quest_target_result",295,new Guid("bfed3fca-9934-374f-b39d-cebb0729e57f"),new[]{null,new QuestTargetResultMessage("",101,true,true,120,30,2),Obj<QuestTargetResultMessage>("CharacterName",null,"GoalID",-1,"IsGoalSuccess",false,"IsQuestSuccess",true,"Exp",int.MaxValue,"Gold",int.MinValue,"Ap",0)},cases);
  AddCases("quest_success_scene_start",550,new Guid("62db5582-deb3-34e7-98e2-3fd6692deca6"),new[]{null,new QuestSuccessSceneStartMessage()},cases);
  var summary=new QuestSummaryInfo("quest_gnoll_scout",-1,MicroPlayResult.Success){IsTodayQuest=true,LastProgress=1,CurrentProgress=2,Exp=150,Gold=40,Ap=3};
  summary.ClearedGoals.Add(101,new SummaryGoalInfo(101,100,30,2));summary.ClearedGoals.Add(-7,new SummaryGoalInfo(-7,0,0,0));
  summary.BattleExp.Add(new SummaryRewardInfo("MonsterKill",0,0,50));summary.EtcExp.Add(new SummaryRewardInfo(null,-1,int.MaxValue,int.MinValue));summary.MainGold.Add(new SummaryRewardInfo("Goal_é",1,2,3));
  summary.RewardItem.Add("javelin_lvl0",1);summary.RewardItem.Add("",-5);
  var bare=new QuestSummaryInfo(null,0,MicroPlayResult.Exception){BattleExp=null,EtcExp=null,MainGold=null,EtcGold=null,QuestAp=null,EtcAp=null,RewardItem=null,ClearedGoals=null};
  AddCases("finish_game",460,new Guid("4bc1c4f0-07af-3449-9246-f7a5e68f76a2"),new[]{null,new FinishGameMessage(null,false,false,false),new FinishGameMessage(summary,false,true,true),new FinishGameMessage(bare,true,false,true)},cases);
  AddCases("return_to_town",445,new Guid("702a1018-f1b5-3c8e-ad51-e46ef0846aed"),new[]{null,new ReturnToTownMessage()},cases);
  AddCases("return_from_quest",402,new Guid("6764d890-113e-3d4d-9d96-24c0087489c3"),new[]{null,new ReturnFromQuestMessage(ReturnFromQuestType.ToTown),Obj<ReturnFromQuestMessage>("Order",2,"StorySectorBSP",null,"StorySectorEntity","story_é")},cases);
  AddCases("kicked",471,new Guid("4df74867-68cb-3431-8177-ce7549890ddd"),new[]{null,new KickedMessage()},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Utility {
 public static class Log<T> { public static readonly DummyLogger Logger=new DummyLogger(); }
 public class DummyLogger { public void DebugFormat(string f,params object[] a){} }
 public static class Extention {
  static readonly DateTime epoch=new DateTime(1970,1,1,0,0,0,DateTimeKind.Utc);
  public static int ToInt32(this DateTime value){DateTime d=value.Kind==DateTimeKind.Local?value.ToUniversalTime():value;return (int)((d-epoch).Ticks/10000000);}
  public static DateTime ToDateTime(this int value){return epoch+new TimeSpan(0,0,value);}
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
// GiveItem.ResultEnum as recovered (GetItemMessage's constructor parameter); the GiveItem
// operation itself depends on the service framework and is not compiled here.
namespace ServiceCore.ItemServiceOperations { public class GiveItem { public enum ResultEnum { Success, Mailed_UniqueViolation, Mailed_NoEmptySlot, Mailed_Unknown, Failed } } }
