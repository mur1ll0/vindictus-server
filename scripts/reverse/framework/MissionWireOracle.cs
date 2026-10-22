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
