using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class AllUserGoalWireOracle {
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
 static void Main(){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var cases=new List<object>();
  AddCases("all_user_goal_event",52,new Guid("8531c095-3d0a-3e5f-8e05-d60f0c5ade95"),new[]{null,
   new AllUserGoalEventMessage(new Dictionary<int,int>{{589,12},{601,-1},{int.MaxValue,int.MinValue}}),
   new AllUserGoalEventMessage(new Dictionary<int,int>()),
   new AllUserGoalEventMessage(null)},cases);
  AddCases("all_user_goal_event_modify",53,new Guid("ad82781d-ee67-38c2-9674-626448d02d94"),new[]{null,new AllUserGoalEventModifyMessage(589,150),new AllUserGoalEventModifyMessage(int.MinValue,-2)},cases);
  AddCases("all_user_goal_reward",54,new Guid("22836c87-a7f6-333b-aa9c-dd0987da4bb6"),new[]{null,new AllUserGoalRewardMessage(589),new AllUserGoalRewardMessage(-1)},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
