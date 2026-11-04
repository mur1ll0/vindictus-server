using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class WhisperWireOracle {
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
  AddCases("whisper_filter_list",636,new Guid("9d9c801a-4700-3cd3-889e-99b210174432"),new[]{null,
   Obj<WhisperFilterListMessage>("Filter",new Dictionary<string,int>{{"Alice",1},{"Bob",0},{"Ünïcøde",int.MinValue}}),
   Obj<WhisperFilterListMessage>("Filter",new Dictionary<string,int>()),
   Obj<WhisperFilterListMessage>("Filter",null)},cases);
  AddCases("update_whisper_filter",637,new Guid("a7953e9f-47e2-3ec3-9aac-c9839f26427d"),new[]{null,
   Obj<UpdateWhisperFilterMessage>("OperationType",1,"TargetID","Alice","Level",1),
   Obj<UpdateWhisperFilterMessage>("OperationType",0,"TargetID",null,"Level",-1),
   Obj<UpdateWhisperFilterMessage>("OperationType",int.MinValue,"TargetID","Ünïcøde","Level",int.MaxValue)},cases);
  AddCases("request_whisper",638,new Guid("5e3ee58f-1ed5-3582-b197-aa2273d6b334"),new[]{null,
   Obj<RequestWhisperMessage>("Receiver","Alice","Contents","hello"),
   Obj<RequestWhisperMessage>("Receiver",null,"Contents",null),
   Obj<RequestWhisperMessage>("Receiver","","Contents",""),
   Obj<RequestWhisperMessage>("Receiver","Ünïcøde","Contents","olá 日本")},cases);
  AddCases("whisper",639,new Guid("ac053a13-a83c-322a-9903-e0f1f68d1287"),new[]{null,
   Obj<WhisperMessage>("Sender","Bob","Contents","hello"),
   Obj<WhisperMessage>("Sender",null,"Contents",null),
   Obj<WhisperMessage>("Sender","Ünïcøde","Contents","olá 日本")},cases);
  AddCases("whisper_fail",640,new Guid("905bd8c4-246c-302c-9f46-c9a7b8927a87"),new[]{null,
   new WhisperFailMessage("Alice","GameUI_Heroes_Chat_Whisper_Not_Online"),
   new WhisperFailMessage(null,null),
   new WhisperFailMessage("","GameUI_Heroes_Chat_Whisper_Invalid_Character_Name")},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
