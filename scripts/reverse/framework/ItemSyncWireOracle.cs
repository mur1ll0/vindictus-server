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
class ItemSyncWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v is string || v.GetType().IsPrimitive)return v;
  if(v is IDictionary){var rows=new List<object>();foreach(DictionaryEntry x in (IDictionary)v) rows.Add(new {key=Capture(x.Key),value=Capture(x.Value)});return rows;}
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  var props=v.GetType().GetProperties().Where(p=>p.CanRead).ToArray();
  if(props.Length==0)return v.GetType().GetFields(BindingFlags.Instance|BindingFlags.NonPublic|BindingFlags.Public).ToDictionary(f=>f.Name,f=>Capture(f.GetValue(v)));
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
 static T Priv<T>(string field,object value) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  typeof(T).GetField(field,BindingFlags.Instance|BindingFlags.NonPublic).SetValue(obj,value);
  return obj;
 }
 static CostumeInfo Costume(int seed) {
  var c=new CostumeInfo();
  c.CostumeTypeInfo=new Dictionary<int,int>{{0,seed},{int.MaxValue,-1}};
  c.ColorInfo=new Dictionary<int,int>{{int.MinValue,seed}};
  c.AvatarInfo=new Dictionary<int,bool>{{1,true},{2,false}};
  c.AvatarHideInfo=null;
  c.PollutionInfo=new Dictionary<int,byte>{{3,255}};
  c.EffectInfo=new Dictionary<int,int>();
  c.DecorationInfo=new Dictionary<int,int>{{4,seed}};
  c.DecorationColorInfo=new Dictionary<int,int>{{5,-1}};
  c.Shineness=seed;c.Height=int.MinValue;c.Bust=int.MaxValue;c.PaintingPosX=1;c.PaintingPosY=-1;c.PaintingRotation=seed;c.PaintingSize=2;
  c.HideHeadCostume=seed!=0;c.CafeType=seed;c.IsReturn=seed==0;c.VIPCode=seed;c.BodyPaintingPosX=3;c.BodyPaintingPosY=4;c.BodyPaintingRotation=5;c.BodyPaintingSize=6;c.BodyPaintingSide=7;c.BodyPaintingClip=8;c.BodyPaintingMode=9;
  c.BodyShapeInfo=new Dictionary<int,float>{{0,1.5f},{1,float.NaN},{2,-0.0f}};
  c.TownEffect=seed;
  return c;
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
  var updates=new List<UpdateInventoryInfoMessage>{null,Priv<UpdateInventoryInfoMessage>("slotInfos",null),new UpdateInventoryInfoMessage(new List<SlotInfo>()),new UpdateInventoryInfoMessage(new List<SlotInfo>{null}),new UpdateInventoryInfoMessage(new List<SlotInfo>{Obj<SlotInfo>()})};
  foreach(int seed in new[]{0,-1,1,int.MinValue,int.MaxValue})updates.Add(new UpdateInventoryInfoMessage(new List<SlotInfo>{Slot(seed),null,Slot(seed^1)}));
  AddCases("update_inventory_info",617,new Guid("abacd3bc-1a84-3570-a62c-f1b0007675fb"),updates,cases);
  var fails=new List<ItemFailMessage>{null,new ItemFailMessage(null),new ItemFailMessage(Obj<HeroesString>()),new ItemFailMessage(new HeroesString("GameUI_Heroes_EquipItemFail_System")),new ItemFailMessage(new HeroesString("GameUI_Heroes_ConstraintFail_LowLevel",new string[]{null,"","値"})),new ItemFailMessage(new HeroesString(null,new string[0]))};
  AddCases("item_fail",358,new Guid("2f756b90-8fba-38b8-bcb2-a51f0f132502"),fails,cases);
  var costumes=new List<CostumeUpdateMessage>{null,new CostumeUpdateMessage(null),new CostumeUpdateMessage(new CostumeInfo())};
  foreach(int seed in new[]{0,-1,1,int.MinValue,int.MaxValue})costumes.Add(new CostumeUpdateMessage(Costume(seed)));
  AddCases("costume_update",447,new Guid("099b778f-141e-359d-b270-60c9d2f99bbb"),costumes,cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Utility { public static class Log<T> { public static readonly DummyLogger Logger=new DummyLogger(); } public class DummyLogger {public void DebugFormat(string f,params object[] a){} } }
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
