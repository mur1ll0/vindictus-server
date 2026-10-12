using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Threading;
using System.Web.Script.Serialization;
using ServiceCore.ItemServiceOperations;
public static class Case {
 public static List<string> Args=new List<string>();
 public static int Resolve(string text) { Args.Add(Program.Hex(text)); int n;if(int.TryParse(text,out n))return n;return text.Length*17+text.Sum(c=>(int)c)%101; }
}
namespace Utility {
 public static class Ext { public static V TryGetValue<K,V>(this IDictionary<K,V> d,K k){V v;return d.TryGetValue(k,out v)?v:default(V);} }
 public static class Log<T> { public static Logger Logger=new Logger(); }
 public class Logger { public void WarnFormat(string f,params object[] a){} }
}
public class Program {
 public static string Hex(string text) {
  if(text==null)return null;
  return String.Concat(text.Select(c=>((byte)c).ToString("x2")+((byte)(c>>8)).ToString("x2")));
 }
 static object Attr(ItemAttributeElement a){return a==null?null:new{name_hex=Hex(a.AttributeName),value_hex=Hex(a.Value),arg=a.Arg,arg2=a.Arg2};}
 public static void Main(){
  var inputs=new List<string>{null,"","item","item[]"," item [ A : value ]","item[A:1,A:2]","item[A:1,a:2]","[A:1]","item,A:1","item[A:1]tail","item[A::1]","item[:]", "item[A: ]", "item[A:x(1)<2>]","item[A:x<2>(1)]", "item[A:x(1)(2)]", "item[A:x(1<2>)]", "item[A:x(1]","item[A:x<2]","item[A:x)junk]","item[A:x>junk]", "item[A:x(1):z]","item[QUALITY:(5)]","item[ENHANCE:9]","item[A:\ud800]", "\ud800[A:x]","item[A:x\0]","item[A\0:x]","item[A:x(-2147483648)<2147483647>]","item[A:x(0)<0>]","item[A:x(-1)<-1>]"};
  foreach(var w in new[]{" ","\t","\u0085","\u00a0","\u1680","\u2000","\u200a","\u2028","\u2029","\u202f","\u205f","\u3000","\u200b","\ufeff","\0"}){
   inputs.Add(w+"item["+w+"A"+w+":"+w+"value"+w+"]");inputs.Add("item[A:x("+w+" 1 "+w+")]");
  }
  foreach(var s in new[]{"A","A:","A:x","A::x",":x","A:x(1)","A:x<2>","A:x(1)<2>","A:x(1<2)","A:x)","A:x>", "A:x<2>(1)"}){
   inputs.Add(s);inputs.Add("item["+s+"]");inputs.Add("item["+s+",B:y]");inputs.Add("item["+s+","+s+"]");
  }
  var integerTexts=new List<string>{null,"","0","1","-1","+1","2147483647","2147483648","-2147483648","-2147483649"," 1 ","1\0","1\0\0","1\0 ","1 \0","+","--1","1.0","1,000","\ud800","2020-01-02","1sec","bad","000000000000000000001"};
  foreach(var w in new[]{"\t","\r","\n","\v","\f"," ","\u00a0","\u2000","\0"}){integerTexts.Add(w+"1"+w);}
  var profiles=new List<object>();
  foreach(var culture in new[]{"en-US","pt-BR","zh-TW","tr-TR","fr-FR"}){
   Thread.CurrentThread.CurrentCulture=CultureInfo.GetCultureInfo(culture);
   var cases=new List<object>();var numbers=new List<object>();var pairs=new List<object>();
   foreach(var input in inputs)foreach(var mode in new[]{"class","attributes","single"}){
    Case.Args.Clear();string klass=null,built=null;object[] attrs=null;bool error=false;
    try{
     if(mode=="single"){
      var a=ItemClassExBuilder.ParseSingleAttribute(input);
      attrs=a==null?new object[0]:new[]{Attr(a)};
     }else{
      Dictionary<string,ItemAttributeElement> a;
      if(mode=="class")ItemClassExBuilder.Parse(input,out klass,out a);
      else ItemClassExBuilder.Parse(input,out a);
      attrs=a.Values.Select(Attr).ToArray();
      if(mode=="class")built=ItemClassExBuilder.Build(klass,a);
     }
    }catch(Exception){error=true;}
    cases.Add(new{input_hex=Hex(input),mode,error,class_hex=Hex(klass),attrs,built_hex=Hex(built),arguments=Case.Args.ToArray()});
   }
   foreach(var input in integerTexts){int n;bool ok=int.TryParse(input,out n);numbers.Add(new{input_hex=Hex(input),success=ok,result=ok?(int?)n:null});}
   foreach(var left in new[]{"item[A:1,B:2]","item[A:1]","item","item[A:x(1)]"})
   foreach(var right in new[]{"item[B:2,A:1]","item[A:1]","Item[A:1]","item","item[A:x(1)]","item[A:x<1>]"}){
    string l,r;Dictionary<string,ItemAttributeElement> la,ra;
    ItemClassExBuilder.Parse(left,out l,out la);ItemClassExBuilder.Parse(right,out r,out ra);
    pairs.Add(new{left_hex=Hex(left),right_hex=Hex(right),same=ItemClassExBuilder.IsSameItemClassEx(l,la,r,ra)});
   }
   profiles.Add(new{culture,cases,numbers,pairs});
  }
  var locations=new List<object>();
  foreach(byte collection in new byte[]{0,1,100,101,102,103,254,255})
   foreach(int slot in new[]{int.MinValue,-2,-1,0,1,int.MaxValue}){
    var l=new PlayerService.ItemServiceCore.ItemInstance.ItemLocation(collection,slot);
    locations.Add(new{collection,slot,is_null=l.IsNull,equal_null=l.Equals(PlayerService.ItemServiceCore.ItemInstance.ItemLocation.NullLocation)});
   }
  Console.Write(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==8?"x64":"x86",profiles,locations}));
 }
}
