using System;using System.Collections.Generic;using System.Linq;using System.IO;using System.Text;using System.Globalization;using System.Security.Cryptography;using System.Web.Script.Serialization;
public class FeatureRow {public string key,value;}
public class RawEquip {public string item_utf16,feature;}
public class RawClass : RawEquip {public int max_stack;public byte storage;public string bind,trade_on_bind;}
public class Package {public FeatureRow[] features;public RawEquip[] equips,extras;public RawClass[] classes;}
public class EquipItemInfo {public string ItemClass,Feature;public int SourceIndex;}
public class ItemClassInfo {public string ItemClass,Feature,Bind,TradeOnBind;public int MaxStack,SourceIndex;public byte Storage;}
public class ItemClassExtraInfo {public string ItemClass,Feature;public int SourceIndex;}
public class QuietLog {public void ErrorFormat(string format,params object[] args){Case.Invalid++;}}
public static class Log<T> {public static QuietLog Logger=new QuietLog();}
public static class HeroesContentsLoader {
 public static IEnumerable<T> GetTable<T>() where T:class {
  if(typeof(T)==typeof(EquipItemInfo))return (IEnumerable<T>)(object)Case.Equips;
  if(typeof(T)==typeof(ItemClassInfo))return (IEnumerable<T>)(object)Case.Classes;
  return (IEnumerable<T>)(object)Case.Extras;
 }
}
public static class Case {
 public static Dictionary<string,string> Features;public static int Phase,Invalid;public static int[] EnabledCount=new int[5];
 public static List<EquipItemInfo> Equips;public static List<ItemClassInfo> Classes;public static List<ItemClassExtraInfo> Extras;
 public static List<string> Calls=new List<string>();
 public static bool Enabled(string expression){Calls.Add(Phase+":"+(expression??"<null>"));bool enabled=ServiceCore.FeatureMatrix.IsEnable(expression);if(enabled)EnabledCount[Phase]++;return enabled;}
 public static object Run(Package p,bool trace){
  Features=p.features.ToDictionary(f=>f.key,f=>f.value??"");Invalid=0;EnabledCount=new int[5];Calls.Clear();
  Equips=p.equips.Select((r,i)=>new EquipItemInfo{ItemClass=Program.Decode(r.item_utf16),Feature=r.feature,SourceIndex=i}).ToList();
  Classes=p.classes.Select((r,i)=>new ItemClassInfo{ItemClass=Program.Decode(r.item_utf16),Feature=r.feature,MaxStack=r.max_stack,Storage=r.storage,Bind=r.bind,TradeOnBind=r.trade_on_bind,SourceIndex=i}).ToList();
  Extras=p.extras.Select((r,i)=>new ItemClassExtraInfo{ItemClass=Program.Decode(r.item_utf16),Feature=r.feature,SourceIndex=i}).ToList();
  object summary=null;string error=null;
  try {ItemContents.LoadProjection();summary=new {counts=new {equip_rows=p.equips.Length,equip_enabled=EnabledCount[1],equip_unique=ItemContents.equipItemInfoDic.Count,class_rows=p.classes.Length,class_disabled=p.classes.Length-EnabledCount[2],class_invalid=Invalid,class_accepted=EnabledCount[2]-Invalid,class_unique=ItemContents.itemClassInfoDic.Count,class_overwrites=EnabledCount[2]-Invalid-ItemContents.itemClassInfoDic.Count,extra_rows=p.extras.Length,extra_enabled=EnabledCount[3],extra_unique=ItemContents.itemClassExtraInfoDic.Count,storage_remapped=ItemContents.itemClassInfoDic.Values.Count(i=>p.classes[i.SourceIndex].storage==5 && i.Storage==1)},projection_sha256=Program.Digest(),full_startup_verified=false,equipment_stats_loaded=false,random_rates_loaded=false,database_mutations=false,game_authentication_available=false};}
  catch(ArgumentNullException){error="NullKey";}catch(IndexOutOfRangeException){error="Feature";}
  return new {summary=summary,error=error,calls=trace ? Calls.ToArray():null};
 }
}
public static class Program {
 public static byte[] Unicode(string s){var b=new byte[s.Length*2];for(int i=0;i<s.Length;i++){b[2*i]=(byte)s[i];b[2*i+1]=(byte)(s[i]>>8);}return b;}
 public static string Hex(string s){return s==null?null:BitConverter.ToString(Unicode(s)).Replace("-","").ToLowerInvariant();}
 public static string Decode(string hex){if(hex==null)return null;var c=new char[hex.Length/4];for(int i=0;i<c.Length;i++)c[i]=(char)(Convert.ToByte(hex.Substring(i*4,2),16)|(Convert.ToByte(hex.Substring(i*4+2,2),16)<<8));return new string(c);}
 static void Field(BinaryWriter w,byte[] b){w.Write((byte)(b==null?0:1));if(b!=null){w.Write(b.Length);w.Write(b);}}
 public static string Digest(){
  using(var s=new MemoryStream())using(var w=new BinaryWriter(s)){
   w.Write(Encoding.ASCII.GetBytes("ItemCatalogProjection\0v1"));
   foreach(var row in ItemContents.equipItemInfoDic.OrderBy(x=>Hex(x.Key),StringComparer.Ordinal)){w.Write((byte)'E');Field(w,Unicode(row.Key));w.Write(row.Value.SourceIndex);}
   foreach(var row in ItemContents.itemClassInfoDic.OrderBy(x=>Hex(x.Key),StringComparer.Ordinal)){var i=row.Value;w.Write((byte)'I');Field(w,Unicode(row.Key));w.Write(i.SourceIndex);w.Write(i.MaxStack);w.Write(i.Storage);w.Write((byte)(ItemContents.equipItemInfoDic.ContainsKey(row.Key)?1:0));Field(w,i.Bind==null?null:Encoding.UTF8.GetBytes(i.Bind));Field(w,i.TradeOnBind==null?null:Encoding.UTF8.GetBytes(i.TradeOnBind));}
   foreach(var row in ItemContents.itemClassExtraInfoDic.OrderBy(x=>Hex(x.Key),StringComparer.Ordinal)){w.Write((byte)'X');Field(w,Unicode(row.Key));w.Write(row.Value.SourceIndex);}
   using(var sha=SHA256.Create())return BitConverter.ToString(sha.ComputeHash(s.ToArray())).Replace("-","").ToLowerInvariant();
  }
 }
 static Package Package(bool story,RawEquip[] equips,RawClass[] classes,RawEquip[] extras){return new Package{features=new[]{new FeatureRow{key="CurrentVer",value="3.09.00"},new FeatureRow{key="on",value="1.1"},new FeatureRow{key="off",value=""},new FeatureRow{key="StoryTabInInventory",value=story?"1":""}},equips=equips,classes=classes,extras=extras};}
 static RawEquip E(string key,string feature){return new RawEquip{item_utf16=Hex(key),feature=feature};}
 static RawClass I(string key,string feature,int stack,byte storage,string bind=null,string trade="off"){return new RawClass{item_utf16=Hex(key),feature=feature,max_stack=stack,storage=storage,bind=bind,trade_on_bind=trade};}
 static object Test(Package p){return new {package=p,observed=Case.Run(p,true)};}
 public static void Main(string[] args){
  var serializer=new JavaScriptSerializer{MaxJsonLength=16*1024*1024};var actual=serializer.Deserialize<Package>(File.ReadAllText(args[0]));
  var old=serializer.Deserialize<Dictionary<string,object>[]>(File.ReadAllText(args[1]));var known=new HashSet<string>(old.Select(p=>(string)p["current"]+"\0"+(string)p["value"]));
  var versions=actual.features.Where(f=>f.key!="CurrentVer" && f.key!="BraceletMaxSlotCount" && !string.IsNullOrEmpty(f.value) && !known.Contains(actual.features.Single(f2=>f2.key=="CurrentVer").value+"\0"+f.value)).Select(f=>f.value).Distinct().OrderBy(v=>v,StringComparer.Ordinal).ToArray();
  var pairs=new List<object>();string current=actual.features.Single(f=>f.key=="CurrentVer").value;
  foreach(string culture in new[]{"","en-US","zh-TW","zh-CN","tr-TR"}) {CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);foreach(string value in versions)pairs.Add(new {culture=culture,current=current,value=value,enabled=current.CompareTo(value)>=0});}
  CultureInfo.CurrentCulture=new CultureInfo("en-US",false);
  var cases=new List<object>();
  foreach(bool story in new[]{false,true})foreach(string key in new[]{"study","STUDY","gold","", "study\0","study\ud800","stüdy"})foreach(string eqFeature in new string[]{null,"","on","off"})foreach(int stack in new[]{int.MinValue,-1,0,1,2,int.MaxValue})
   cases.Add(Test(Package(story,new[]{E(key,eqFeature)},new[]{I(key,"on",stack,5)},new[]{E(key,"on")})));
  foreach(bool story in new[]{false,true})foreach(int stack in new[]{0,1,2})foreach(string first in new[]{"on","off"})foreach(string second in new[]{"on","off"})
   cases.Add(Test(Package(story,new[]{E("study","on"),E("study","off"),E("STUDY","on")},new[]{I("study",first,1,5,"on","off"),I("study",second,stack,2,"off",null),I("STUDY","on",1,5)},new[]{E("study","on"),E("study","off"),E("study","on")})));
  foreach(byte storage in new byte[]{0,1,4,5,6,255})foreach(bool story in new[]{false,true})cases.Add(Test(Package(story,new RawEquip[0],new[]{I("study","on",1,storage)},new RawEquip[0])));
  foreach(int phase in new[]{1,2,3})foreach(string feature in new string[]{null,"off"," "})foreach(int stack in new[]{0,1}) {
   var p=Package(false,phase==1?new[]{E(null,feature)}:new RawEquip[0],phase==2?new[]{I(null,feature,stack,5)}:new RawClass[0],phase==3?new[]{E(null,feature)}:new RawEquip[0]);cases.Add(Test(p));
  }
  cases.Add(Test(Package(false,new RawEquip[0],new RawClass[0],new RawEquip[0])));
  var actualResult=Case.Run(actual,false);
  Console.Write(serializer.Serialize(new {architecture=IntPtr.Size==8?"x64":"x86",culture="en-US",version_pairs=pairs,cases=cases,actual=actualResult}));
 }
}
