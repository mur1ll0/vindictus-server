using System;
using System.Linq;
using System.Collections;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.CharacterServiceOperations;
class FullCharacterListWireOracle {
 static int[] Body(Packet p){return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray();}
 static object Capture(object v){
  if(v==null)return null;
  if(v is float)return new{bits=BitConverter.ToUInt32(BitConverter.GetBytes((float)v),0)};
  if(v.GetType().IsEnum)return Convert.ToInt32(v);
  if(v is string||v is int||v is long||v is bool||v is byte)return v;
  if(v is IDictionary){var rows=new List<object>();foreach(DictionaryEntry d in (IDictionary)v)rows.Add(new{key=Capture(d.Key),value=Capture(d.Value)});return rows;}
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  return v.GetType().GetProperties().ToDictionary(p=>p.Name,p=>Capture(p.GetValue(v,null)));
 }
 static CharacterSummary Summary(int seed,bool detailed){
  var ctor=typeof(CharacterSummary).GetConstructors().Single();
  var args=ctor.GetParameters().Select((p,i)=>p.ParameterType==typeof(string)?(object)(p.Name=="characterID"?"Murael":null):p.ParameterType==typeof(int)?unchecked(seed+i*101):p.ParameterType==typeof(long)?(seed==int.MinValue?long.MinValue:seed==int.MaxValue?long.MaxValue:(long)seed):p.ParameterType==typeof(bool)?(object)(((seed+i)&1)==0):p.ParameterType.IsEnum?Enum.ToObject(p.ParameterType,seed):null).ToArray();
  var c=(CharacterSummary)ctor.Invoke(args);
  if(detailed){
   var x=c.Costume;
   x.CostumeTypeInfo.Add(seed,~seed);x.ColorInfo.Add(-1,seed);x.AvatarInfo.Add(seed,true);x.AvatarHideInfo.Add(0,-1);x.PollutionInfo.Add(seed,255);x.EffectInfo.Add(1,seed);x.DecorationInfo.Add(seed,7);x.DecorationColorInfo.Add(7,seed);x.BodyShapeInfo.Add(seed,BitConverter.ToSingle(BitConverter.GetBytes(0x7fc01234u),0));x.BodyShapeInfo.Add(~seed,-0f);
   foreach(var p in typeof(CostumeInfo).GetProperties()){if(p.PropertyType==typeof(int))p.SetValue(x,seed,null);if(p.PropertyType==typeof(bool))p.SetValue(x,(seed&1)==0,null);}
  }
  return c;
 }
 static void Main(){
  Guid guid=new Guid("f2750037-d9ab-3b34-9697-e1626e224040");if(typeof(CharacterListMessage).GUID!=guid)throw new InvalidOperationException("Original identity mismatch");
  var values=new List<CharacterListMessage>{null};
  foreach(int seed in new[]{int.MinValue,-1,0,1,20,int.MaxValue})foreach(bool detail in new[]{false,true}){
   var c=Summary(seed,detail);
   foreach(var chars in new IList<CharacterSummary>[] {new[]{c},new CharacterSummary[]{null,c,Summary(~seed,!detail)}})
    values.Add(new CharacterListMessage(chars,seed,unchecked(seed+1),~seed,true,seed,new[]{seed,-1}));
  }
  foreach(var p in typeof(CharacterSummary).GetProperties()){
   if(p.Name=="Pet")continue;
   var c=Summary(0,false);object v=p.PropertyType==typeof(string)?(object)"lambda-λ😀":p.PropertyType==typeof(long)?long.MinValue:p.PropertyType==typeof(int)?int.MaxValue:p.PropertyType==typeof(bool)?(object)true:p.PropertyType.IsEnum?Enum.ToObject(p.PropertyType,7):null;
   p.SetValue(c,v,null);values.Add(new CharacterListMessage(new[]{c},6,0,0,false,0,null));
  }
  foreach(var p in typeof(CostumeInfo).GetProperties()){
   var c=Summary(0,false);object v=p.PropertyType==typeof(int)?(object)int.MaxValue:p.PropertyType==typeof(bool)?(object)true:null;
   if(p.PropertyType.IsGenericType){
    var element=p.PropertyType.GetGenericArguments()[1];var d=(IDictionary)Activator.CreateInstance(typeof(Dictionary<,>).MakeGenericType(typeof(int),element));
    object value=element==typeof(int)?(object)int.MinValue:element==typeof(bool)?(object)true:element==typeof(byte)?(object)(byte)235:BitConverter.ToSingle(BitConverter.GetBytes(0x7fc01234u),0);d.Add(-7,value);v=d;
   }
   p.SetValue(c.Costume,v,null);values.Add(new CharacterListMessage(new[]{c},6,0,0,false,0,null));
  }
  var generic=values.Select(v=>SerializeWriter.ToBinary<CharacterListMessage>(v)).ToArray();ClassInfo<CharacterListMessage>.CategoryId=479;
  var cases=new List<object>();for(int n=0;n<values.Count;n++){
   var typed=SerializeWriter.ToBinary<CharacterListMessage>(values[n]);
   foreach(string mode in new[]{"unregistered-generic","registered-object","registered-typed"}){
    var packet=mode=="unregistered-generic"?generic[n]:mode=="registered-object"?SerializeWriter.ToBinary((object)values[n]):typed;
    CharacterListMessage decoded;SerializeReader.FromBinary<CharacterListMessage>(packet,out decoded);
    if(!Body(SerializeWriter.ToBinary<CharacterListMessage>(decoded)).SequenceEqual(Body(typed)))throw new InvalidOperationException("Recovered roundtrip differs");
    cases.Add(new{mode,category=mode=="registered-typed"?479:0,body=Body(packet),typed_body=Body(typed),fields=Capture(decoded)});
   }
  }
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=16*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
