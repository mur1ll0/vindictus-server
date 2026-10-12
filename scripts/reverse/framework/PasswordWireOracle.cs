using System;
using System.Linq;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class PasswordWireOracle {
 static int[] Body(Packet p){return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray();}
 static void Cases<T>(string kind,int category,Guid guid,IEnumerable<T> values,List<object> cases) where T:class {
  if(typeof(T).GUID!=guid)throw new InvalidOperationException("Pinned type identity differs");
  var messages=values.ToArray();var generic=messages.Select(v=>SerializeWriter.ToBinary<T>(v)).ToArray();
  ClassInfo<T>.CategoryId=category;
  for(int n=0;n<messages.Length;n++){
   var typed=SerializeWriter.ToBinary<T>(messages[n]);
   foreach(var mode in new[]{"unregistered-generic","registered-object","registered-typed"}){
    var packet=mode=="unregistered-generic"?generic[n]:mode=="registered-object"?SerializeWriter.ToBinary((object)messages[n]):typed;
    T decoded;SerializeReader.FromBinary<T>(packet,out decoded);
    if(!Body(SerializeWriter.ToBinary<T>(decoded)).SequenceEqual(Body(typed)))throw new InvalidOperationException("Recovered roundtrip differs");
    object fields=null;
    if(decoded is SecondPasswordMessage)fields=new{password=((SecondPasswordMessage)(object)decoded).Password};
    if(decoded is SetSecondPasswordMessage)fields=new{password=((SetSecondPasswordMessage)(object)decoded).NewPassword};
    if(decoded is HasSecondPasswordMessage){var v=(HasSecondPasswordMessage)(object)decoded;fields=new{first=v.IsFirstQuery,has=v.HasSecondPassword,passed=v.IsPassed,fail=v.FailCount,retry=v.RetryLockedSec};}
    if(decoded is SecondPasswordResultMessage){var v=(SecondPasswordResultMessage)(object)decoded;fields=new{operation=(int)v.OperationType,passed=v.Passed,fail=v.FailCount,retry=v.RetryLockedSec};}
    cases.Add(new{kind,mode,category=mode=="registered-typed"?category:0,body=Body(packet),typed_body=Body(typed),is_null=decoded==null,fields});
   }
  }
 }
 static void Main(){
  var cases=new List<object>();
  Cases("ask",231,new Guid("e9e7d6a1-2131-3b59-826d-88c968ae0867"),new AskSecondPasswordMessage[]{null,new AskSecondPasswordMessage()},cases);
  var passwords=new string[]{null,"","123456","Lab-É-λ-😀",new string('a',127),new string('a',128),new string('a',256),"a\0b"};
  Cases("check",405,new Guid("1f3f118d-7775-3b45-b608-a383ba381ed6"),new SecondPasswordMessage[]{null}.Concat(passwords.Select(p=>new SecondPasswordMessage(p))),cases);
  Cases("set",572,new Guid("0c9611cc-7fe0-3aea-8e2b-35b3d8904786"),new SetSecondPasswordMessage[]{null}.Concat(passwords.Select(p=>new SetSecondPasswordMessage(p))),cases);
  var has=new List<HasSecondPasswordMessage>{null};foreach(bool first in new[]{false,true})foreach(bool present in new[]{false,true})foreach(bool passed in new[]{false,true})foreach(int fail in new[]{int.MinValue,-1,0,5,int.MaxValue})has.Add(new HasSecondPasswordMessage(first,present,passed,fail,fail));
  Cases("has",413,new Guid("95739ac9-26a7-325e-9f4b-b63c40dc590c"),has,cases);
  var results=new List<SecondPasswordResultMessage>{null};foreach(int operation in new[]{int.MinValue,-1,0,1,2,3,4,int.MaxValue})foreach(bool passed in new[]{false,true})foreach(int fail in new[]{int.MinValue,-1,0,5,int.MaxValue})results.Add(new SecondPasswordResultMessage((SecondPasswordResultMessage.ProcessType)operation,passed,fail,fail));
  Cases("result",562,new Guid("b2b1f8d6-4c60-325e-854f-2f73550d8e9b"),results,cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=4*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
