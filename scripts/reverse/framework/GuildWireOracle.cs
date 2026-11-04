using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.EndPointNetwork.GuildService;
class GuildWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v.GetType().IsEnum)return Convert.ToInt32(v);
  if(v is string || v.GetType().IsPrimitive)return v;
  if(v is IDictionary)return ((IDictionary)v).Keys.Cast<object>().Select(k=>new{key=Capture(k),value=Capture(((IDictionary)v)[k])}).ToArray();
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  return v.GetType().GetProperties().Where(p=>p.CanRead).ToDictionary(p=>p.Name,p=>Capture(p.GetValue(v,null)));
 }
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
   foreach(string mode in new[]{"unregistered-generic","registered-object","registered-typed"}){
    var packet=mode=="unregistered-generic"?generic[n]:mode=="registered-object"?SerializeWriter.ToBinary((object)items[n]):typed;
    T decoded;
    SerializeReader.FromBinary<T>(packet,out decoded);
    var reencoded=SerializeWriter.ToBinary<T>(decoded);
    if(!Body(reencoded).SequenceEqual(Body(typed)))throw new InvalidOperationException("Roundtrip mismatch for "+kind+" case "+n);
    var analyzer=new MessageAnalyzer();
    var raw=packet.Bytes;
    var copy=new byte[raw.Count];
    Buffer.BlockCopy(raw.Array,raw.Offset,copy,0,raw.Count);
    analyzer.Add(new ArraySegment<byte>(copy));
    var analyzed=analyzer.ToArray();
    if(analyzed.Length!=1)throw new InvalidOperationException("MessageAnalyzer count mismatch");
    var framed=new Packet(analyzed[0]);
    if(framed.CategoryId!=packet.CategoryId||!Body(framed).SequenceEqual(Body(packet)))throw new InvalidOperationException("MessageAnalyzer payload mismatch");
    cases.Add(new{kind,mode,category=packet.CategoryId,body=Body(packet),typed_body=Body(typed),is_null=decoded==null,fields=Capture(decoded)});
   }
  }
 }

 static InGameGuildInfo Info(int sn,string name,string master,Dictionary<byte,int> gp){
  return Obj<InGameGuildInfo>("GuildSN",sn,"GuildName",name,"GuildLevel",3,"MemberCount",21,"MasterName",master,"MaxMemberCount",100,"IsNewbieRecommend",true,"GuildPoint",long.MaxValue,"GuildNotice","Bem-vindos à guilda","DailyGainGP",gp);
 }
 static GuildMemberInfo Member(string name,int level,int rank,long point,int days,bool online){
  return Obj<GuildMemberInfo>("CharacterName",name,"GameLevel",level,"Rank",rank,"Point",point,"LastLoginTime",days,"IsOnline",online);
 }
 static void Main(){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var cases=new List<object>();
  AddCases("open_guild",338,new Guid("37c909f5-c8f6-33d2-8f2c-7dda5b1fe08d"),new[]{null,
   Obj<OpenGuildMessage>("GuildName","Night_Owls","GuildNameID","nightowls","GuildIntro","Olá"),
   Obj<OpenGuildMessage>("GuildName",null,"GuildNameID",null,"GuildIntro",null)},cases);
  AddCases("guild_result",281,new Guid("4d108cd7-8273-3e5b-afcc-2d077d73e7e0"),new[]{null,
   new GuildResultMessage(GuildResultEnum.ConnectSuccess),
   new GuildResultMessage(GuildResultEnum.OpenGuildSuccess,123456789012L),
   new GuildResultMessage(GuildResultEnum.OpenGuildFail,"GuildDuplicatedName"),
   new GuildResultMessage(GuildResultEnum.JoinGuildChatFail,(string)null)},cases);
  AddCases("guild_info",278,new Guid("647a5f98-b588-328f-b58d-ddc8164d3014"),new[]{null,
   new GuildInfoMessage(null),
   new GuildInfoMessage(Info(1001,"Night_Owls","Alice",new Dictionary<byte,int>{{0,200},{1,60},{255,int.MinValue}})),
   new GuildInfoMessage(Info(-1,null,null,null)),
   new GuildInfoMessage(Info(7,"","",new Dictionary<byte,int>()))},cases);
  AddCases("guild_member_list",336,new Guid("e9a63382-19a4-39b9-915a-765a485a84b0"),new[]{null,
   Obj<GuildMemberListMessage>("IsFullUpdate",true,"Members",new List<GuildMemberInfo>{Member("Alice",60,1,1500,0,true),Member("Böb",1,4,0,365,false),Member(null,int.MinValue,0,long.MinValue,-1,false)}),
   Obj<GuildMemberListMessage>("IsFullUpdate",false,"Members",new List<GuildMemberInfo>()),
   Obj<GuildMemberListMessage>("IsFullUpdate",false,"Members",null)},cases);
  AddCases("query_guild_list",339,new Guid("34da3a6d-e9a2-3f2f-83a6-e73965a7b636"),new[]{null,
   Obj<QueryGuildListMessage>("QueryType",0,"SearchKey","Night","Page",1,"PageSize",(byte)10),
   Obj<QueryGuildListMessage>("QueryType",-1,"SearchKey",null,"Page",int.MaxValue,"PageSize",(byte)255)},cases);
  AddCases("guild_list",279,new Guid("d74f17bb-616b-3a80-a17f-46217557ad03"),new[]{null,
   Obj<GuildListMessage>("GuildList",new List<InGameGuildInfo>{Info(1001,"Night_Owls","Alice",new Dictionary<byte,int>()),Info(1002,"Day_Larks","Éva",null)},"Page",1,"TotalPage",3),
   Obj<GuildListMessage>("GuildList",null,"Page",0,"TotalPage",0)},cases);
  AddCases("join_guild",284,new Guid("7abea10d-5632-3388-bea1-c342d53465f8"),new[]{null,Obj<JoinGuildMessage>("GuildSN",1001),Obj<JoinGuildMessage>("GuildSN",int.MinValue)},cases);
  AddCases("leave_guild",285,new Guid("eb9a9076-bd3a-3358-87b9-14a00200e1f8"),new[]{null,Obj<LeaveGuildMessage>()},cases);
  AddCases("close_guild",277,new Guid("89f37dba-2fc1-3b7d-9512-465648c48278"),new[]{null,Obj<CloseGuildMessage>()},cases);
  AddCases("reconnect_guild",286,new Guid("918435ec-3213-3b76-b1b8-569b1f7657cb"),new[]{null,Obj<ReconnectGuildMessage>()},cases);
  AddCases("guild_operation",337,new Guid("0c77318b-cd6a-3bd6-8d6c-1e26f68f8717"),new[]{null,
   Obj<GuildOperationMessage>("Operations",new List<GuildOperationInfo>{Obj<GuildOperationInfo>("Command","Join","Target","Bob","Value",1),Obj<GuildOperationInfo>("Command","Rank","Target","Carol","Value",0),Obj<GuildOperationInfo>("Command",null,"Target",null,"Value",int.MinValue)}),
   Obj<GuildOperationMessage>("Operations",new List<GuildOperationInfo>()),
   Obj<GuildOperationMessage>("Operations",null)},cases);
  AddCases("change_master",84,new Guid("8f635448-72bd-3685-bcea-b6cd22d0db8d"),new[]{null,Obj<ChangeMasterMessage>("NewMasterName","Bob"),Obj<ChangeMasterMessage>("NewMasterName",null)},cases);
  AddCases("invite_guild",283,new Guid("51b65671-7ce1-3ca7-8572-1b02fb80076a"),new[]{null,Obj<InviteGuildMessage>("Name","Bob"),Obj<InviteGuildMessage>("Name",null)},cases);
  AddCases("guild_notice_edit",280,new Guid("3df39210-4621-3c4a-b85a-a1c864c4f86d"),new[]{null,Obj<GuildNoticeEditMessage>("Text","Raide às 21h"),Obj<GuildNoticeEditMessage>("Text",null)},cases);
  AddCases("guild_story_request",282,new Guid("efc67527-b0af-310f-a8b7-b39c1922e670"),new[]{null,Obj<GuildStoryRequestMessage>()},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
