using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.RankServiceOperations;
class RankWireOracle {
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
 static RankResultInfo R(int e,int r,int p,long s,string n){return new RankResultInfo(e,r,p,s,n);}
 static void Main(){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var cases=new List<object>();
  AddCases("query_random_rank",147,new Guid("051924be-e16a-3172-9a8b-0e65b13edb6f"),new QueryRandomRankInfoMessage[]{null,Obj<QueryRandomRankInfoMessage>()},cases);
  AddCases("random_rank",148,new Guid("7411ad56-8be6-3a81-bdd3-a69f242c3fe2"),new RandomRankInfoMessage[]{null,Obj<RandomRankInfoMessage>("RandomRankResult",new List<RandomRankResultInfo>{new RandomRankResultInfo(new List<RankResultInfo>{R(60003,1,0,12,"Alice"),R(60003,2,1,9,"")},60003,2),new RandomRankResultInfo(null,0,1)}),Obj<RandomRankInfoMessage>("RandomRankResult",null)},cases);
  AddCases("query_rank_alarm",333,new Guid("81ccd497-b4c1-35d7-b08c-77ddf2073089"),new QueryRankAlarmInfoMessage[]{null,Obj<QueryRankAlarmInfoMessage>("CID",123456789012L)},cases);
  AddCases("rank_alarm",334,new Guid("b1f6d4f1-e607-30fa-915f-1e5e1c14ed0c"),new RankAlarmInfoMessage[]{null,Obj<RankAlarmInfoMessage>("RankAlarm",new List<RankAlarmInfo>{new RankAlarmInfo(1,20001,7),new RankAlarmInfo(3,int.MaxValue,int.MinValue)}),Obj<RankAlarmInfoMessage>("RankAlarm",null)},cases);
  AddCases("query_rank_all",382,new Guid("76e547c7-7cf9-3523-96be-0d5367e969aa"),new QueryRankAllInfoMessage[]{null,Obj<QueryRankAllInfoMessage>("PeriodType",2)},cases);
  AddCases("query_rank_character",383,new Guid("d2180bc7-06fd-38e3-95e9-7801bcb01860"),new QueryRankCharacterInfoMessage[]{null,Obj<QueryRankCharacterInfoMessage>("CID",5L,"PeriodType",1)},cases);
  AddCases("query_rank_favorites",384,new Guid("22ab80f4-a54c-3a02-8a37-daad929429a9"),new QueryRankFavoritesInfoMessage[]{null,Obj<QueryRankFavoritesInfoMessage>("CID",long.MinValue,"PeriodType",3)},cases);
  AddCases("query_rank_goal",385,new Guid("05c1e7bd-6448-31a6-9798-2d9c2cbec28e"),new QueryRankGoalInfoMessage[]{null,Obj<QueryRankGoalInfoMessage>("CID",5L,"EventID",50001,"PeriodType",1)},cases);
  AddCases("query_rank_heavy_favorites",386,new Guid("ecdf4e69-d15e-37a0-a873-534ca32e79dd"),new QueryRankHeavyFavoritesInfoMessage[]{null,Obj<QueryRankHeavyFavoritesInfoMessage>()},cases);
  AddCases("query_rank",387,new Guid("9a9b066c-03e8-3ebf-a2ab-e171a2b6665d"),new QueryRankInfoMessage[]{null,Obj<QueryRankInfoMessage>("RankID","SoloTimeAttack_quest_a","PeriodType",2),Obj<QueryRankInfoMessage>("RankID",null,"PeriodType",0)},cases);
  AddCases("query_rank_other",388,new Guid("46da8595-f780-3157-86e8-ed68f230bd60"),new QueryRankOtherCharacterInfoMessage[]{null,Obj<QueryRankOtherCharacterInfoMessage>("RequesterCID",5L,"CharacterName","B\u00f6b","PeriodType",1),Obj<QueryRankOtherCharacterInfoMessage>("RequesterCID",0L,"CharacterName",null,"PeriodType",3)},cases);
  AddCases("rank_character",389,new Guid("069fe707-679c-3fad-a66f-f2cae935c923"),new RankCharacterInfoMessage[]{null,Obj<RankCharacterInfoMessage>("RankResult",new List<RankResultInfo>{R(50001,3,4,120,""),R(20001,1,0,long.MaxValue,null)},"PeriodType",1),Obj<RankCharacterInfoMessage>("RankResult",null,"PeriodType",2)},cases);
  AddCases("rank_favorites",390,new Guid("bb38aa48-cb2a-3db0-a98a-862a571abda7"),new RankFavoritesInfoMessage[]{null,Obj<RankFavoritesInfoMessage>("RankResult",new List<RankResultInfo>{R(50001,0,0,0,"")},"PeriodType",3)},cases);
  AddCases("rank_goal",391,new Guid("5652a397-6ea3-3b22-9c37-7c6969a8902f"),new RankGoalInfoMessage[]{null,Obj<RankGoalInfoMessage>("EventID",50001,"RivalName","Carol","RivalScore",99L,"Rank",2,"RankPrev",3),Obj<RankGoalInfoMessage>("EventID",0,"RivalName",null,"RivalScore",0L,"Rank",0,"RankPrev",0)},cases);
  AddCases("rank_heavy_favorites",392,new Guid("ff2edfd7-44a3-356d-9f05-6dcfbd9447a1"),new RankHeavyFavoritesInfoMessage[]{null,Obj<RankHeavyFavoritesInfoMessage>("RankResult",new List<RankResultInfo>{R(60003,1,2,0,"")})},cases);
  AddCases("rank_all",393,new Guid("61439335-cfa2-35b9-9d41-d26c4778dd90"),new RankAllInfoMessage[]{null,Obj<RankAllInfoMessage>("RankResult",new List<RankResultInfo>{R(10002,1,1,345678,"Alice"),R(60001,1,0,4,"Night_Owls")},"PeriodType",1)},cases);
  AddCases("rank_info",394,new Guid("7d4ee0f1-991f-3ca7-ae3d-2fd9ec4be411"),new RankInfoMessage[]{null,Obj<RankInfoMessage>("RankResult",new List<RankResultInfo>(),"PeriodType",2)},cases);
  AddCases("rank_other",395,new Guid("70712a6c-a1c3-330a-bb24-c8bb9d0b6830"),new RankOtherCharacterInfoMessage[]{null,Obj<RankOtherCharacterInfoMessage>("Level",60,"BaseCharacter",3,"CharacterName","Bob","RankResult",new List<RankResultInfo>{R(50001,5,5,10,"")},"RequesterRankResult",null,"PeriodType",1)},cases);
  AddCases("set_rank_favorite",396,new Guid("8824e598-669b-3370-87a4-1407030f5525"),new RequestSetRankFavoriteInfoMessage[]{null,Obj<RequestSetRankFavoriteInfoMessage>("CID",5L,"RankID","GamePlay_complete_battle","IsAddition",true),Obj<RequestSetRankFavoriteInfoMessage>("CID",0L,"RankID",null,"IsAddition",false)},cases);
  AddCases("set_rank_goal",397,new Guid("af164256-d1f0-311f-9894-896845db1f36"),new RequestSetRankGoalInfoMessage[]{null,Obj<RequestSetRankGoalInfoMessage>("CID",5L,"RankID","x")},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
