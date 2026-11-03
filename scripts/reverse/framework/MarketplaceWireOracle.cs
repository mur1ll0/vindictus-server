using System;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class MarketplaceWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v is DateTime)return new {ticks=((DateTime)v).Ticks,kind=(int)((DateTime)v).Kind};
  if(v.GetType().IsEnum)return Convert.ToInt32(v);
  if(v is string || v.GetType().IsPrimitive)return v;
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
 static TradeItemInfo Item(long tid,string name,string cls,long count,long price,int close,string attr,byte type){
  return Obj<TradeItemInfo>("TID",tid,"CID",tid*3,"ChracterName",name,"ItemClass",cls,"ItemCount",count,"ItemPrice",price,"CloseDate",close,"AttributeEX",attr,"MaxArmorCondition",-7,"color1",-1,"color2",int.MaxValue,"color3",int.MinValue,"MinPrice",10,"MaxPrice",90,"AvgPrice",55,"TradeType",type);
 }
 static List<DetailOption> Options(){
  return new List<DetailOption>{new DetailOption("PRICE",5000,1),new DetailOption(null,int.MinValue,255),new DetailOption("COLOR1R",128,10)};
 }
 static void Main(){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var cases=new List<object>();
  var items=new List<TradeItemInfo>{Item(1,"Fiona","gold",1500,1,604800,"",0),null,Item(long.MaxValue,null,null,long.MinValue,long.MaxValue,int.MinValue,null,255),Item(-5,"ペット","sword",1,99999999,0,"ENHANCE:9,QUALITY:(4)",1)};
  AddCases("trade_category_search",305,new Guid("77610fcf-fc87-3633-8f8e-de283eff2e6f"),new[]{null,
   Obj<TradeCategorySearchMessage>("tradeCategory","WEAPON","tradeCategorySub","LONGSWORD","minLevel",1,"maxLevel",90,"uniqueNumber",7,"ChunkPageNumber",1,"Order",SortOrder.Level,"isDescending",false,"DetailOptions",new List<DetailOption>()),
   Obj<TradeCategorySearchMessage>("tradeCategory",null,"tradeCategorySub",null,"minLevel",int.MinValue,"maxLevel",int.MaxValue,"uniqueNumber",-1,"ChunkPageNumber",0,"Order",(SortOrder)99,"isDescending",true,"DetailOptions",null),
   Obj<TradeCategorySearchMessage>("tradeCategory","","tradeCategorySub","","minLevel",0,"maxLevel",0,"uniqueNumber",0,"ChunkPageNumber",int.MaxValue,"Order",SortOrder.Price,"isDescending",true,"DetailOptions",Options())},cases);
  AddCases("trade_item_class_list_search",306,new Guid("92300b56-a6fc-35ec-aa0c-c344e2615e6f"),new[]{null,
   Obj<TradeItemClassListSearchMessage>("ItemClassList",new List<string>{"sword","potion_hp_small"},"uniqueNumber",3,"ChunkPageNumber",2,"Order",SortOrder.Price,"isDescending",false,"DetailOptions",new List<DetailOption>()),
   Obj<TradeItemClassListSearchMessage>("ItemClassList",null,"uniqueNumber",int.MinValue,"ChunkPageNumber",-1,"Order",SortOrder.Level,"isDescending",true,"DetailOptions",null),
   Obj<TradeItemClassListSearchMessage>("ItemClassList",new List<string>{null,"",new string('a',60)},"uniqueNumber",int.MaxValue,"ChunkPageNumber",1,"Order",(SortOrder)0,"isDescending",false,"DetailOptions",Options())},cases);
  AddCases("trade_cancel_my_item",302,new Guid("63d59401-6b18-305a-af2f-98a77d2d977b"),new[]{null,Obj<TradeCancelMyItemMessage>("TID",1L,"UniqueNumber",5),Obj<TradeCancelMyItemMessage>("TID",long.MinValue,"UniqueNumber",int.MaxValue)},cases);
  AddCases("trade_cancel_my_item_result",303,new Guid("bbae86b3-29cf-3b81-afd5-5736213f0c2d"),new[]{null,Obj<TradeCancelMyItemResultMessage>("UniqueNumber",5,"Result",0),Obj<TradeCancelMyItemResultMessage>("UniqueNumber",int.MinValue,"Result",4)},cases);
  AddCases("trade_request_my_item",311,new Guid("e0188571-a1a3-3b93-8772-9c44ad8d1114"),new[]{null,new TradeRequestMyItemMessage()},cases);
  AddCases("trade_my_item_infos",308,new Guid("1d431b50-2a5d-3e6e-829f-8c1a5057b5b3"),new[]{null,
   Obj<TradeMyItemInfos>("TradeItemList",null,"result",7),
   Obj<TradeMyItemInfos>("TradeItemList",new List<TradeItemInfo>(),"result",0),
   Obj<TradeMyItemInfos>("TradeItemList",items,"result",int.MinValue)},cases);
  AddCases("trade_purchase_item",309,new Guid("6d284225-0676-3001-9369-ba59fe31cb4f"),new[]{null,Obj<TradePurchaseItemMessage>("TID",9L,"PurchaseCount",3,"UniqueNumber",1),Obj<TradePurchaseItemMessage>("TID",long.MaxValue,"PurchaseCount",int.MinValue,"UniqueNumber",-1)},cases);
  AddCases("trade_purchase_item_result",310,new Guid("813c7eaf-a144-334e-81cd-9d066f11000e"),new[]{null,Obj<TradePurchaseItemResultMessage>("UniqueNumber",1,"Result",0,"LeftNumber",7),Obj<TradePurchaseItemResultMessage>("UniqueNumber",int.MaxValue,"Result",9,"LeftNumber",int.MinValue)},cases);
  AddCases("trade_search_result",312,new Guid("f6cbae04-367a-3926-b010-aaed1b28790a"),new[]{null,
   Obj<TradeSearchResult>("TradeItemList",null,"UniqueNumber",2,"IsMoreResult",false,"result",8),
   Obj<TradeSearchResult>("TradeItemList",items,"UniqueNumber",int.MinValue,"IsMoreResult",true,"result",0)},cases);
  AddCases("insert_trade_order",427,new Guid("5dd5d7fb-1389-353f-8bcf-7d2fbd9edb82"),new[]{null,
   Obj<InsertTradeOrderMessage>("ItemID",638448480000016391L,"Num",3,"DurationMin",4320,"UnitPrice",1500,"TradeType",(byte)0),
   Obj<InsertTradeOrderMessage>("ItemID",long.MinValue,"Num",int.MinValue,"DurationMin",int.MaxValue,"UnitPrice",-1,"TradeType",(byte)255)},cases);
  AddCases("insert_trade_order_result",428,new Guid("7ab66731-d827-3eee-bcec-1c50b5a066fc"),new[]{null,Obj<InsertTradeOrderResultMessage>("Result",0),Obj<InsertTradeOrderResultMessage>("Result",11),Obj<InsertTradeOrderResultMessage>("Result",int.MinValue)},cases);
  AddCases("insert_trade_tir_order",429,new Guid("97f6218c-8733-3b4b-bdd7-af26cf089413"),new[]{null,Obj<InsertTradeTirOrderMessage>("Num",1,"DurationMin",1440,"UnitPrice",300),Obj<InsertTradeTirOrderMessage>("Num",int.MinValue,"DurationMin",-1,"UnitPrice",int.MaxValue)},cases);
  AddCases("start_trade_session",592,new Guid("99350e05-2420-364a-a3bc-4be82c6f1cf7"),new[]{null,new StartTradeSessionMessage()},cases);
  AddCases("end_trade_session",594,new Guid("84675e2c-42fe-3895-9f1f-b913dd2d1918"),new[]{null,new EndTradeSessionMessage()},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
