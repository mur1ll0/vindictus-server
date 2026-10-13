using System;
using System.Linq;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.CharacterServiceOperations;
class EmptyCharacterListWireOracle {
 static int[] Body(Packet p){return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray();}
 static void Main(){
  Guid guid=new Guid("f2750037-d9ab-3b34-9697-e1626e224040");if(typeof(CharacterListMessage).GUID!=guid)throw new InvalidOperationException("Original identity mismatch");
  var values=new List<CharacterListMessage>{null};
  foreach(bool present in new[]{false,true})foreach(var party in new int[][]{null,new int[0],new[]{int.MinValue,0,1,int.MaxValue}})foreach(int count in new[]{int.MinValue,-1,0,20,int.MaxValue})foreach(bool played in new[]{false,true})
    values.Add(new CharacterListMessage(present?new List<CharacterSummary>():null,count,unchecked(count+1),~count,played,count^0x55AA55AA,party));
  var generic=values.Select(v=>SerializeWriter.ToBinary<CharacterListMessage>(v)).ToArray();ClassInfo<CharacterListMessage>.CategoryId=479;
  var cases=new List<object>();for(int n=0;n<values.Count;n++){
   var typed=SerializeWriter.ToBinary<CharacterListMessage>(values[n]);
   foreach(string mode in new[]{"unregistered-generic","registered-object","registered-typed"}){
    var packet=mode=="unregistered-generic"?generic[n]:mode=="registered-object"?SerializeWriter.ToBinary((object)values[n]):typed;
    CharacterListMessage decoded;SerializeReader.FromBinary<CharacterListMessage>(packet,out decoded);
    if(!Body(SerializeWriter.ToBinary<CharacterListMessage>(decoded)).SequenceEqual(Body(typed)))throw new InvalidOperationException("Recovered roundtrip differs");
    object fields=decoded==null?null:new{characters_present=decoded.Characters!=null,characters_count=decoded.Characters==null?-1:decoded.Characters.Count,free=decoded.MaxFreeCharacterCount,purchased=decoded.MaxPurchasedCharacterCount,premium=decoded.MaxPremiumCharacters,prologue=(int)decoded.ProloguePlayed,preset=decoded.PresetUsedCharacterCount,party=decoded.LoginPartyState};
    cases.Add(new{mode,category=mode=="registered-typed"?479:0,body=Body(packet),typed_body=Body(typed),is_null=decoded==null,fields});
   }
  }
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=4*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
