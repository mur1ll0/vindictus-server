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
class GuildStorageWireOracle {
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
  AddCases("guild_inventory_info",87,new Guid("58f0a03b-7aae-3521-bb10-7c39acc518e0"),new[]{null,new GuildInventoryInfoMessage(true,48,1000,7L,new List<SlotInfo>{S(1),null,S(2)}),new GuildInventoryInfoMessage(false,0,int.MinValue,long.MaxValue,null),new GuildInventoryInfoMessage(true,504,0,-1L,new List<SlotInfo>())},cases);
  AddCases("add_guild_storage_item",0,new Guid("3a237aa6-7470-3080-a641-274aac6d4d48"),new[]{null,Obj<AddGuildStorageItemMessage>("ItemID",123456789012L,"ItemClass","potion_é","Amount",5,"TargetTab",(byte)2,"TargetSlot",71),Obj<AddGuildStorageItemMessage>("ItemID",long.MinValue,"ItemClass",null,"Amount",int.MinValue,"TargetTab",(byte)255,"TargetSlot",-1)},cases);
  AddCases("pick_guild_storage_item",0,new Guid("4a24b568-3e57-331b-894b-4f638dbf19b0"),new[]{null,Obj<PickGuildStorageItemMessage>("ItemID",9L,"ItemClass","gold","Amount",int.MaxValue,"TargetTab",(byte)0,"TargetSlot",0),Obj<PickGuildStorageItemMessage>("ItemID",-1L,"ItemClass","","Amount",128,"TargetTab",(byte)128,"TargetSlot",int.MaxValue)},cases);
  AddCases("purchase_guild_storage",0,new Guid("c700797e-33e2-3192-bc91-c83c9131f339"),new[]{null,Obj<PurchaseGuildStorageMessage>("ProductNo",30001,"IsCredit",true),Obj<PurchaseGuildStorageMessage>("ProductNo",int.MinValue,"IsCredit",false)},cases);
  AddCases("arrange_guild_storage_item",0,new Guid("a895bf47-377e-3ad7-a87c-43fea7945f06"),new[]{null,Obj<ArrangeGuildStorageItemMessage>("ItemID",42L,"Slot",503),Obj<ArrangeGuildStorageItemMessage>("ItemID",long.MaxValue,"Slot",int.MinValue)},cases);
  AddCases("handle_guild_storage_session",0,new Guid("bef7785d-6e8a-3a95-a0d9-23cf767ee4f1"),new[]{null,Obj<HandleGuildStorageSessionMessage>("IsStarted",true),Obj<HandleGuildStorageSessionMessage>("IsStarted",false)},cases);
  AddCases("update_guild_storage_setting",0,new Guid("dc4f3599-51af-344d-bca5-deea3f868484"),new[]{null,Obj<UpdateGuildStorageSettingMessage>("GoldLimit",50000,"AccessLimtiTag",0x1ffL),Obj<UpdateGuildStorageSettingMessage>("GoldLimit",-1,"AccessLimtiTag",long.MinValue)},cases);
  var brief=new List<GuildStorageBriefLogElement>{Obj<GuildStorageBriefLogElement>("CharacterName","Alice","OperationType",GuildStorageOperationType.Item,"AddCount",3,"PickCount",1,"Datestamp",20261105,"Timestamp",235959),null,Obj<GuildStorageBriefLogElement>("CharacterName",null,"OperationType",GuildStorageOperationType.Slot,"AddCount",int.MinValue,"PickCount",int.MaxValue,"Datestamp",0,"Timestamp",-1)};
  var items=new List<GuildStorageItemLogElement>{Obj<GuildStorageItemLogElement>("CharacterName","Bob","IsAddItem",true,"ItemClass","potion_é","Count",5,"Datestamp",20261104,"Timestamp",120000,"Color1",-1,"Color2",16777215,"Color3",int.MinValue),Obj<GuildStorageItemLogElement>("CharacterName","","IsAddItem",false,"ItemClass",null,"Count",-5,"Datestamp",int.MaxValue,"Timestamp",int.MinValue,"Color1",0,"Color2",1,"Color3",int.MaxValue)};
  AddCases("guild_storage_logs",0,new Guid("9b995088-968a-3864-8045-5ae4e19e569a"),new[]{null,new GuildStorageLogsMessage(true,brief,items),new GuildStorageLogsMessage(false,null,new List<GuildStorageItemLogElement>()),new GuildStorageLogsMessage(true,new List<GuildStorageBriefLogElement>{Obj<GuildStorageBriefLogElement>("CharacterName","Gold","OperationType",GuildStorageOperationType.Gold,"AddCount",0,"PickCount",0,"Datestamp",1,"Timestamp",2)},null)},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
