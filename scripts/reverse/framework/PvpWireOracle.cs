using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class PvpWireOracle {
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
 static void Main(){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var cases=new List<object>();
  AddCases("free_match_report",276,new Guid("8673db49-f849-349b-acea-3794efe7ece5"),new FreeMatchReportMessage[]{null,new FreeMatchReportMessage(0,1),new FreeMatchReportMessage(3,2),new FreeMatchReportMessage(-1,int.MaxValue),new FreeMatchReportMessage(int.MinValue,128),new FreeMatchReportMessage(127,-128)},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
