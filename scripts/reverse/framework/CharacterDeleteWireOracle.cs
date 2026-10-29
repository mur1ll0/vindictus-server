using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.EndPointNetwork.CharacterList;

class CharacterDeleteWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v is string || v.GetType().IsPrimitive)return v;
  if(v.GetType().IsEnum)return Convert.ToInt32(v);
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  return v.GetType().GetProperties().Where(p=>p.CanRead).ToDictionary(p=>p.Name,p=>Capture(p.GetValue(v,null)));
 }
 static T Obj<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetProperty((string)fields[i]).SetValue(obj,fields[i+1],null);
  return obj;
 }
 static void AddCases<T>(string kind, int category, Guid expectedGuid, IEnumerable<T> values, List<object> cases) where T : class {
  if (typeof(T).GUID != expectedGuid) throw new InvalidOperationException("GUID mismatch for " + typeof(T).FullName);
  var items = values.ToArray();
  var generic = items.Select(v => SerializeWriter.ToBinary<T>(v)).ToArray();
  ClassInfo<T>.CategoryId = category;
  for (int n = 0; n < items.Length; n++) {
   var typed = SerializeWriter.ToBinary<T>(items[n]);
   foreach (string mode in new[] { "unregistered-generic", "registered-object", "registered-typed" }) {
    var packet = mode == "unregistered-generic" ? generic[n] : mode == "registered-object" ? SerializeWriter.ToBinary((object)items[n]) : typed;
    T decoded;
    SerializeReader.FromBinary<T>(packet, out decoded);
    var reencoded = SerializeWriter.ToBinary<T>(decoded);
    if (!Body(reencoded).SequenceEqual(Body(typed))) throw new InvalidOperationException("Roundtrip mismatch for " + kind + " case " + n);
    var analyzer = new MessageAnalyzer();
    var raw = packet.Bytes;
    var copy = new byte[raw.Count];
    Buffer.BlockCopy(raw.Array, raw.Offset, copy, 0, raw.Count);
    analyzer.Add(new ArraySegment<byte>(copy));
    var analyzed = analyzer.ToArray();
    if (analyzed.Length != 1) throw new InvalidOperationException("MessageAnalyzer count mismatch");
    var framed = new Packet(analyzed[0]);
    if (framed.CategoryId != packet.CategoryId || !Body(framed).SequenceEqual(Body(packet))) throw new InvalidOperationException("MessageAnalyzer payload mismatch");
    cases.Add(new { kind, mode, category = packet.CategoryId, body = Body(packet), typed_body = Body(typed), is_null = decoded == null, fields = Capture(decoded) });
   }
  }
 }

 static void Main(){
  var cases=new List<object>();
  var names=new string[]{null,"","Fiona","Ñoño한글"};
  var sns=new[]{int.MinValue,-1,0,1,int.MaxValue};
  var delete=new List<DeleteCharacterMessage>{null};
  var cancel=new List<DeleteCharacterCancelMessage>{null};
  foreach(var name in names)foreach(int sn in sns){delete.Add(Obj<DeleteCharacterMessage>("Name",name,"CharacterSN",sn));cancel.Add(Obj<DeleteCharacterCancelMessage>("Name",name,"CharacterSN",sn));}
  AddCases("delete",451,new Guid("65699026-e61b-38d9-a403-28178a907f59"),delete,cases);
  AddCases("delete_cancel",258,new Guid("7a123c93-9f06-3bba-8e0e-1bf3d7a1d512"),cancel,cases);
  var result=new List<DeleteCharacterResultMessage>{null};
  foreach(int sn in new[]{-1,1,int.MaxValue})foreach(DeleteCharacterResult r in Enum.GetValues(typeof(DeleteCharacterResult)))foreach(int remain in new[]{int.MinValue,0,86400})result.Add(new DeleteCharacterResultMessage(sn,r,remain));
  AddCases("delete_result",407,new Guid("19af38d6-d58a-39e5-bfc7-7b5b88c50e4a"),result,cases);
  var cancelResult=new List<DeleteCharacterCancelResultMessage>{null};
  foreach(int sn in sns)foreach(DeleteCharacterCancelResult r in Enum.GetValues(typeof(DeleteCharacterCancelResult)))cancelResult.Add(new DeleteCharacterCancelResultMessage(sn,r));
  AddCases("delete_cancel_result",259,new Guid("8f6e7d11-8bdc-3e5b-9704-44e95b136338"),cancelResult,cases);
  var orders=new List<SelectOrdersChangeMessage>{null,Obj<SelectOrdersChangeMessage>("SelectOrders",null)};
  foreach(var list in new[]{new byte[0],new byte[]{0},new byte[]{2,0,1},new byte[]{255,0,128,7},Enumerable.Range(0,16).Select(i=>(byte)(15-i)).ToArray()})orders.Add(Obj<SelectOrdersChangeMessage>("SelectOrders",new List<byte>(list)));
  AddCases("select_orders",489,new Guid("d7d63e49-9742-3287-a815-7a0483760e81"),orders,cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Utility { public static class Log<T> { public static readonly DummyLogger Logger=new DummyLogger(); } public class DummyLogger {public void DebugFormat(string f,params object[] a){} } }
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
