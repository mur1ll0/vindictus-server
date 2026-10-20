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
using ServiceCore.ItemServiceOperations;
class InventoryWireOracle {
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
 static SlotInfo Slot(int seed) {
  return Obj<SlotInfo>("ItemID",(long)seed,"ItemClass",seed==0?null:"synthetic_é_装備","Num",seed,"Tab",seed,"Slot",seed,"IsExpireable",seed!=0,"ExpireDateTimeDiff",(long)seed,"ExpireDateTimeTick",long.MaxValue,"Colors",new[]{seed,-1,int.MaxValue},"Durability",seed,"MaxDurability",int.MaxValue,"MaxDurabilityBonus",int.MinValue,"Tradable",true,"ExpanderName","","Attributes",new List<ItemAttributeElement>{null,new ItemAttributeElement("TEST","値",seed,int.MinValue)},"PrefixEnchantStatus",new Dictionary<int,bool>{{-1,true},{7,false}},"SuffixEnchantStatus",new Dictionary<int,bool>(),"MinPrice",int.MinValue,"MaxPrice",int.MaxValue,"AvgPrice",seed,"MinUpTimeDateTimeDiff",long.MinValue);
 }
 static InventoryInfoMessage Inv(int seed) {
  return Obj<InventoryInfoMessage>("StorageInfos",new List<StorageInfo>{null,new StorageInfo(255,true,seed,"",seed),new StorageInfo(0,false,-1,null,0)},"SlotInfos",new List<SlotInfo>{null,Slot(seed)},"EquipmentInfo",new Dictionary<int,long>{{int.MinValue,long.MinValue},{int.MaxValue,long.MaxValue}},"QuickSlotInfo",new QuickSlotInfo(new Dictionary<int,string>{{-1,null},{0,""},{int.MaxValue,"synthetic"}}),"UnequippableParts",new List<int>{int.MinValue,0,int.MaxValue});
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
  AddCases("query_inventory",529,new Guid("efe95078-ef7e-34eb-82e1-2daf3280a576"),new[]{null,new QueryInventoryMessage()},cases);
  var values=new List<InventoryInfoMessage>{null,Obj<InventoryInfoMessage>(),Obj<InventoryInfoMessage>("StorageInfos",new List<StorageInfo>(),"SlotInfos",new List<SlotInfo>(),"EquipmentInfo",new Dictionary<int,long>(),"QuickSlotInfo",new QuickSlotInfo(new Dictionary<int,string>()),"UnequippableParts",new List<int>())};
  foreach(int seed in new[]{0,-1,1,int.MinValue,int.MaxValue})values.Add(Inv(seed));
  foreach(string name in new[]{"StorageInfos","SlotInfos","EquipmentInfo","QuickSlotInfo","UnequippableParts"}){var v=Inv(7);typeof(InventoryInfoMessage).GetProperty(name).SetValue(v,null,null);values.Add(v);}
  values.Add(Obj<InventoryInfoMessage>("SlotInfos",new List<SlotInfo>{Obj<SlotInfo>()},"QuickSlotInfo",new QuickSlotInfo(null)));
  AddCases("inventory_info",468,new Guid("bdc965b5-aca1-3b62-b222-279e96683d93"),values,cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Utility { public static class Log<T> { public static readonly DummyLogger Logger=new DummyLogger(); } public class DummyLogger {public void DebugFormat(string f,params object[] a){} } }
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
