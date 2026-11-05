using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.EndPointNetwork.Pvp;
using ServiceCore.CharacterServiceOperations;
using ServiceCore.MicroPlayServiceOperations;
// PvP match messages (B5 PV2-PV4): the Framework serializer of the recovered ServiceCore types;
// synthetic values only.
class PvpMatchWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static T Obj<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetProperty((string)fields[i]).SetValue(obj,fields[i+1],null);
  return obj;
 }
 static void AddCases<T>(string kind,int category,Guid expectedGuid,IEnumerable<T> values,List<object> cases) where T:class {
  if(typeof(T).GUID!=expectedGuid)throw new InvalidOperationException("GUID mismatch for "+typeof(T).FullName);
  var items=values.ToArray();
  var generic=items.Select(v=>SerializeWriter.ToBinary<T>(v)).ToArray();
  ClassInfo<T>.CategoryId=category;
  for(int n=0;n<items.Length;n++){
   var typed=SerializeWriter.ToBinary<T>(items[n]);
   T decoded;
   SerializeReader.FromBinary<T>(typed,out decoded);
   if(!Body(SerializeWriter.ToBinary<T>(decoded)).SequenceEqual(Body(typed)))throw new InvalidOperationException("Roundtrip mismatch for "+kind+" case "+n);
   cases.Add(new{kind,category=typed.CategoryId,generic_category=generic[n].CategoryId,generic_body=Body(generic[n]),typed_body=Body(typed),is_null=items[n]==null});
  }
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
   var bi=new BattleInventory();bi.Consumables.Add(3,new ConsumablesInfo("potion_hp_lvl1",5,2));bi.Consumables.Add(4,null);g.BattleInventory=bi;
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
 static GameInfo Info(int seed){
  if(seed==0)return Obj<GameInfo>("Name",null,"Map",null,"GameDir",null,"Description",null,"HostID",0,"DSIP",null,"DSPort",0);
  return Obj<GameInfo>("Name","Fiona_é","Map","p05","GameDir","heroes","Description","","HostID",seed,"DSIP","","DSPort",-1);
 }
 static Dictionary<string,string> Config(int seed){
  if(seed==0)return null;
  return new Dictionary<string,string>{{"PvpMode","p05"},{"GameRule","Arena"},{"GameLength","90"},{"BSP",seed==1?"p05":""}};
 }
 static void Main(){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var cases=new List<object>();
  AddCases("query_channel_list",130,new Guid("28e86f3f-74a2-3fc0-a9ae-860806aafb63"),new[]{null,Obj<PvpQueryChannelListMessage>("PvpMode",null,"Key",null),Obj<PvpQueryChannelListMessage>("PvpMode","p05","Key","arena_é")},cases);
  AddCases("channel_list",129,new Guid("a6711fc7-3b8b-35c6-a8a5-2a2d222a9b41"),new[]{null,new PvpChannelListMessage(null,null,null),new PvpChannelListMessage("p05","k",new List<PvpChannelInfo>()),new PvpChannelListMessage("p05","",new List<PvpChannelInfo>{new PvpChannelInfo(0,"0/32"),new PvpChannelInfo(int.MaxValue,null),new PvpChannelInfo(-1,"é")})},cases);
  AddCases("register",442,new Guid("352de823-b4f4-337d-a212-bdfdef5624ce"),new[]{null,Obj<PvpRegisterMessage>("PvpMode",null,"Cheat",(PvpRegisterCheat)0,"ChannelID",0),Obj<PvpRegisterMessage>("PvpMode","p05","Cheat",(PvpRegisterCheat)15,"ChannelID",2),Obj<PvpRegisterMessage>("PvpMode","p03","Cheat",(PvpRegisterCheat)4,"ChannelID",-1)},cases);
  AddCases("unregister",611,new Guid("28670757-a5f2-3ad1-9bde-5031f6606d6c"),new[]{null,new PvpUnregisterMessage()},cases);
  AddCases("confirm_join",143,new Guid("dd1dab96-b042-3e0a-b622-d09b812d90c5"),new[]{null,Obj<PvpConfirmJoinMessage>("PvpMode",null),Obj<PvpConfirmJoinMessage>("PvpMode","p04")},cases);
  AddCases("register_result",378,new Guid("c4cee3b5-48bb-3b17-a6b7-0b05e7c4e803"),new[]{null,Obj<PvpRegisterResultMessage>("PvpMode",null,"UsePendingDialog",false,"StartWaitingTimer",false,"State",0,"Message",null),new PvpRegisterResultMessage("p05",(PvpRegisterResult)1,true,false,"WaitingHost"),new PvpRegisterResultMessage("p03",(PvpRegisterResult)3,true,true,"LevelConstraint_Min",40),Obj<PvpRegisterResultMessage>("PvpMode","","UsePendingDialog",true,"StartWaitingTimer",true,"State",2,"Message",Obj<HeroesString>("Format","DSReady","Parameters",null))},cases);
  AddCases("command",376,new Guid("b27a17ad-9e01-361b-933e-701ccaec625d"),new[]{null,Obj<PvpCommandMessage>("CommandInt",0,"Arg",null),new PvpCommandMessage((PvpCommandType)12,"2,1,0,1"),new PvpCommandMessage((PvpCommandType)10,"")},cases);
  AddCases("kicked",377,new Guid("542086a0-b1ac-3f38-8f87-ea16e3b162c7"),new[]{null,new PvpKickedMessage()},cases);
  AddCases("report",379,new Guid("8e68a8d9-9e06-3199-a19a-9e9cdf506748"),new[]{null,Obj<PvpReportMessage>("EventInt",0,"Subject",0,"Object",0,"Arg",null),Obj<PvpReportMessage>("EventInt",4,"Subject",1,"Object",-1,"Arg","é"),Obj<PvpReportMessage>("EventInt",int.MaxValue,"Subject",int.MinValue,"Object",31,"Arg","")},cases);
  AddCases("team_info",380,new Guid("f851a062-d98a-314d-a92b-060567a6b060"),new[]{null,new PvpTeamInfoMessage(null),new PvpTeamInfoMessage(new Dictionary<int,int>()),new PvpTeamInfoMessage(new Dictionary<int,int>{{0,1},{3,2},{-1,int.MaxValue}})},cases);
  var gameInfos=new[]{null,new PvpGameInfo(new Dictionary<int,string>{{-1,"0, 0"}},new Dictionary<int,string>{{0,"0,0,0,0"},{1,null}}),Obj<PvpGameInfo>("TeamInfo",null,"PlayerInfo",null)};
  AddCases("game_hosted",435,new Guid("b8b23b05-1468-316e-950f-b00af304ca2c"),new[]{null,new PvpGameHostedMessage(null,(PvpTeamID)(-1),-1,null,null),new PvpGameHostedMessage(Join(0),(PvpTeamID)1,0,Config(1),gameInfos[1]),new PvpGameHostedMessage(Join(1),(PvpTeamID)2,31,Config(2),gameInfos[2])},cases);
  AddCases("host_info",436,new Guid("3b54c4d3-e989-3926-b130-9312ae448ff2"),new[]{null,new PvpHostInfoMessage(null,null,null),new PvpHostInfoMessage(Info(0),Join(0),Config(1)),new PvpHostInfoMessage(Info(7),Join(1),new Dictionary<string,string>())},cases);
  AddCases("member_info",437,new Guid("52a61a16-45c8-30e2-a833-9c2a79e3d2d1"),new[]{null,new PvpMemberInfoMessage(null,(PvpTeamID)0),new PvpMemberInfoMessage(Join(1),(PvpTeamID)2)},cases);
  AddCases("arena_result_info",647,new Guid("c36c9bb6-997d-3d08-9a9a-eddab5a6dadf"),new[]{null,new ArenaResultInfoMessage(0,0,0),new ArenaResultInfoMessage(int.MaxValue,-1,7)},cases);
  AddCases("penalty",648,new Guid("6b85dc42-733c-3660-8645-9ed785d1b7e3"),new[]{null,Obj<PvpPenaltyMessage>("PlayerIndex",0),Obj<PvpPenaltyMessage>("PlayerIndex",-1)},cases);
  AddCases("register_game_waiting_queue",0,new Guid("7717e97d-fb9b-3adb-97a2-3be8d0efff52"),new[]{null,Obj<PvpRegisterGameWaitingQueueMessage>("GameIndex",0),Obj<PvpRegisterGameWaitingQueueMessage>("GameIndex",3)},cases);
  AddCases("unregister_game_waiting_queue",0,new Guid("f02655c1-174e-33fd-ad02-563444bf7469"),new[]{null,new PvpUnregisterGameWaitingQueueMessage()},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
// Utility.DictionaryExtension.TryGetValue, used only by the messages' ToString (never called here).
namespace Utility {using System.Collections.Generic;public static class DictionaryExtension{public static V TryGetValue<T,V>(this IDictionary<T,V> d,T key){V v;d.TryGetValue(key,out v);return v;}}}
