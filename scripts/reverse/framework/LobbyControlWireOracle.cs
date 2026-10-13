using System;
using System.Linq;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class LobbyControlWireOracle {
 static int[] Body(Packet p){return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray();}
 static void Cases<T>(string kind,int category,IEnumerable<T> values,List<object> cases) where T:class {
  var messages=values.ToArray();var generic=messages.Select(v=>SerializeWriter.ToBinary<T>(v)).ToArray();ClassInfo<T>.CategoryId=category;
  for(int n=0;n<messages.Length;n++)foreach(var mode in new[]{"unregistered-generic","registered-object","registered-typed"}){
   var typed=SerializeWriter.ToBinary<T>(messages[n]);var packet=mode=="unregistered-generic"?generic[n]:mode=="registered-object"?SerializeWriter.ToBinary((object)messages[n]):typed;
   T decoded;SerializeReader.FromBinary<T>(packet,out decoded);if(!Body(SerializeWriter.ToBinary<T>(decoded)).SequenceEqual(Body(typed)))throw new InvalidOperationException("Recovered roundtrip differs");
   object fields=null;
   if(decoded is PingMessage)fields=new{tag=((PingMessage)(object)decoded).Tag};
   if(decoded is PongMessage)fields=new{tag=((PongMessage)(object)decoded).Tag};
   if(decoded is NGSecurityMessage){var v=(NGSecurityMessage)(object)decoded;fields=new{data=v.message==null?null:v.message.Select(b=>(int)b).ToArray(),checksum=v.checkSum};}
   if(decoded is SystemMessage){var v=(SystemMessage)(object)decoded;fields=new{category=v.Category,text_present=v.Message!=null,format=v.Message==null?null:v.Message.Format,parameters=v.Message==null?null:v.Message.Parameters};}
   if(decoded is CafeLoginMessage){var v=(CafeLoginMessage)(object)decoded;var t=typeof(CafeLoginMessage);var flags=System.Reflection.BindingFlags.NonPublic|System.Reflection.BindingFlags.Instance;fields=new{home=(int)t.GetField("HomeCafeRemainTime",flags).GetValue(v),account=(int)t.GetField("AccountHomeCafeRemainTime",flags).GetValue(v),cafe=(int)t.GetField("CafeType",flags).GetValue(v)};}
   cases.Add(new{kind,mode,category=mode=="registered-typed"?category:0,body=Body(packet),typed_body=Body(typed),is_null=decoded==null,fields});
  }
 }
 static void Main(){var cases=new List<object>();var tags=new[]{int.MinValue,-1,0,1,int.MaxValue};
 Cases("ping",521,new PingMessage[]{null}.Concat(tags.Select(t=>new PingMessage(t))),cases);Cases("pong",523,new PongMessage[]{null}.Concat(tags.Select(t=>new PongMessage(t))),cases);
 Cases("cafe",236,new CafeLoginMessage[]{null,new CafeLoginMessage(0,0,4)}.Concat(tags.Select(t=>new CafeLoginMessage(t,~t,t^0x55aa55aa))),cases);
 var texts=new HeroesString[]{null,new HeroesString(null),new HeroesString(""),new HeroesString("GameUI_Heroes_SecondPassword_Locked"),new HeroesString("GameUI_Heroes_DormantAccount_Alarm_Text"),new HeroesString("Lab-É-λ-😀"),new HeroesString("format",(string[])null),new HeroesString("format",new[]{"one","two"}),new HeroesString("format",new string[]{null,"","λ"})};
 var system=new List<SystemMessage>{null};foreach(byte c in new byte[]{0,1,2,3,4,5,6,7,8,9,10,255})foreach(var h in texts)system.Add(new SystemMessage((SystemMessageCategory)c,h));Cases("system",602,system,cases);
 var ng=new List<NGSecurityMessage>{null};foreach(var data in new byte[][]{null,new byte[0],new byte[]{0},new byte[]{1,2,255},Enumerable.Range(0,41).Select(i=>(byte)i).ToArray(),Enumerable.Range(0,8192).Select(i=>(byte)i).ToArray()})foreach(ulong sum in new[]{0UL,1UL,ulong.MaxValue})ng.Add(new NGSecurityMessage{message=data,checkSum=sum});Cases("ng",16,ng,cases);
 Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=4*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
