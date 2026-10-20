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

class EquipmentWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v is string || v.GetType().IsPrimitive)return v;
  if(v is IDictionary){var rows=new List<object>();foreach(DictionaryEntry x in (IDictionary)v) rows.Add(new {key=Capture(x.Key),value=Capture(x.Value)});return rows;}
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  return v.GetType().GetProperties().Where(p=>p.CanRead && p.Name!="Color1" && p.Name!="Color2" && p.Name!="Color3").ToDictionary(p=>p.Name,p=>Capture(p.GetValue(v,null)));
 }
 static T Obj<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetProperty((string)fields[i]).SetValue(obj,fields[i+1],null);
  return obj;
 }
    static void AddCases<T>(string kind, int category, Guid expectedGuid, IEnumerable<T> values, List<object> cases) where T : class {
        if (typeof(T).GUID != expectedGuid) {
            throw new InvalidOperationException("GUID mismatch for " + typeof(T).FullName);
        }
        var items = values.ToArray();
        var generic = items.Select(v => SerializeWriter.ToBinary<T>(v)).ToArray();
        ClassInfo<T>.CategoryId = category;
        for (int n = 0; n < items.Length; n++) {
            var typed = SerializeWriter.ToBinary<T>(items[n]);
            foreach (string mode in new[] { "unregistered-generic", "registered-object", "registered-typed" }) {
                var packet = mode == "unregistered-generic" ? generic[n]
                    : mode == "registered-object" ? SerializeWriter.ToBinary((object)items[n])
                    : typed;
                T decoded;
                SerializeReader.FromBinary<T>(packet, out decoded);
                var reencoded = SerializeWriter.ToBinary<T>(decoded);
                if (!Body(reencoded).SequenceEqual(Body(typed))) {
                    throw new InvalidOperationException("Roundtrip mismatch for " + kind + " case " + n);
                }
                // Verify MessageAnalyzer (unencrypted channel transport) frames and deframes packet.Bytes identically.
                var analyzer = new MessageAnalyzer();
                var raw = packet.Bytes;
                var copy = new byte[raw.Count];
                Buffer.BlockCopy(raw.Array, raw.Offset, copy, 0, raw.Count);
                analyzer.Add(new ArraySegment<byte>(copy));
                var analyzed = analyzer.ToArray();
                if (analyzed.Length != 1) {
                    throw new InvalidOperationException("MessageAnalyzer count mismatch");
                }
                var framed = new Packet(analyzed[0]);
                if (framed.CategoryId != packet.CategoryId || !Body(framed).SequenceEqual(Body(packet))) {
                    throw new InvalidOperationException("MessageAnalyzer payload mismatch");
                }
                cases.Add(new {
                    kind,
                    mode,
                    category = packet.CategoryId,
                    body = Body(packet),
                    typed_body = Body(typed),
                    is_null = decoded == null,
                    fields = Capture(decoded)
                });
            }
        }
    }

 static void Main(){
 var cases=new List<object>();
 var items=new List<EquipItemMessage>{null};
 foreach(long id in new[]{long.MinValue,-1L,0L,1L,long.MaxValue})foreach(int part in new[]{int.MinValue,-1,0,255,int.MaxValue})items.Add(new EquipItemMessage(id,part));
 AddCases("equip_item",457,new Guid("a6551816-3ca1-38a2-8ec0-7ecba2ac7f36"),items,cases);
 AddCases("equipment_info",458,new Guid("9df997b4-ce70-3558-86a1-7ee1ea91325c"),new[]{null,new EquipmentInfoMessage(null),new EquipmentInfoMessage(new Dictionary<int,long>()),new EquipmentInfoMessage(new Dictionary<int,long>{{int.MinValue,long.MinValue},{int.MaxValue,long.MaxValue},{0,-1}})},cases);
 Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Utility { public static class Log<T> { public static readonly DummyLogger Logger=new DummyLogger(); } public class DummyLogger {public void DebugFormat(string f,params object[] a){} } }
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
