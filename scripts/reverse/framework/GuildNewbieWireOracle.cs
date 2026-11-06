using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.ItemServiceOperations;
using ServiceCore.EndPointNetwork.GuildService;
class GuildNewbieWireOracle {
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
 // Category 0 types are only sent unregistered (the TypeConverter GUID envelope).
 static void AddCases<T>(string kind,int category,Guid expectedGuid,IEnumerable<T> values,List<object> cases) where T:class {
  if(typeof(T).GUID!=expectedGuid)throw new InvalidOperationException("GUID mismatch for "+typeof(T).FullName);
  var items=values.ToArray();
  var generic=items.Select(v=>SerializeWriter.ToBinary<T>(v)).ToArray();
  if(category!=0)ClassInfo<T>.CategoryId=category;
  for(int n=0;n<items.Length;n++){
   var typed=SerializeWriter.ToBinary<T>(items[n]);
   var modes=category==0?new[]{"unregistered-generic"}:new[]{"unregistered-generic","registered-object","registered-typed"};
   foreach(string mode in modes){
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
 static SlotInfo S(int seed){return Obj<SlotInfo>("ItemID",(long)seed,"ItemClass",seed==0?null:"guild_item_é","Num",seed,"Tab",seed%3,"Slot",seed,"IsExpireable",seed!=0,"ExpireDateTimeDiff",(long)seed,"ExpireDateTimeTick",long.MaxValue,"Colors",new[]{seed,-1,int.MaxValue},"Durability",seed,"MaxDurability",int.MaxValue,"MaxDurabilityBonus",int.MinValue,"Tradable",true,"ExpanderName","","Attributes",new List<ItemAttributeElement>{null,new ItemAttributeElement("TEST","値",seed,int.MinValue)},"PrefixEnchantStatus",new Dictionary<int,bool>{{-1,true},{seed,false}},"SuffixEnchantStatus",null,"MinPrice",seed,"MaxPrice",int.MaxValue,"AvgPrice",-seed,"MinUpTimeDateTimeDiff",long.MinValue);}
 static void Main(){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var cases=new List<object>();
  var info=Obj<InGameGuildInfo>("GuildSN",7,"GuildName","Night_Owls","GuildLevel",3,"MemberCount",2,"MasterName","Fiona_é","MaxMemberCount",50,"IsNewbieRecommend",true,"GuildPoint",1234L,"GuildNotice","","DailyGainGP",new Dictionary<byte,int>{{0,10},{1,600}});
  AddCases("find_newbie_recommend_guild",0,new Guid("1a16885d-f10c-3778-b694-bb3ec0448294"),new[]{null,new FindNewbieRecommendGuildMessage()},cases);
  AddCases("find_newbie_recommend_guild_result",0,new Guid("99f557b5-d1f3-3c4e-b5a2-9063c3dc1a9b"),new[]{null,Obj<FindNewbieRecommendGuildResultMessage>("info",null),Obj<FindNewbieRecommendGuildResultMessage>("info",info)},cases);
  AddCases("request_newbie_recommend_guild",0,new Guid("d84b903e-f448-3d7e-87b1-1d506f1dedcc"),new[]{null,Obj<RequestNewbieRecommendGuildMessage>("GuildSN",0),Obj<RequestNewbieRecommendGuildMessage>("GuildSN",int.MaxValue)},cases);
  AddCases("notice_newbie_recommend",0,new Guid("35251c7c-a752-3f8f-b8d5-b8c875311683"),new[]{null,Obj<NoticeNewbieRecommendMessage>("RequestUserName",null,"ShipID",0L),Obj<NoticeNewbieRecommendMessage>("RequestUserName","Evie_é","ShipID",long.MaxValue)},cases);
  AddCases("direct_purchase_guild_item",0,new Guid("dbc653b2-4d96-3906-bfff-4849fa819fe1"),new[]{null,Obj<DirectPurchaseGuildItemMessage>("ProductNo",0,"IsCredit",false,"ItemClass",null),Obj<DirectPurchaseGuildItemMessage>("ProductNo",-1,"IsCredit",true,"ItemClass","guild_extension_member_10")},cases);
  AddCases("direct_purchase_guild_item_result",0,new Guid("4e5db23c-be88-3d8d-b9c6-07d2bdc040f9"),new[]{null,Obj<DirectPurchaseGuildItemResultMessage>("ItemClass",null,"Result",0),Obj<DirectPurchaseGuildItemResultMessage>("ItemClass","guild_extension_member_10","Result",1)},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
