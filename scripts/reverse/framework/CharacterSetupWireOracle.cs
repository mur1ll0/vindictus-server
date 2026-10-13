using System;using System.Linq;using System.Collections.Generic;using System.Web.Script.Serialization;using Devcat.Core.Net.Message;using ServiceCore.EndPointNetwork;
class CharacterSetupWireOracle {
 static int[] Body(Packet p){return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray();}
 static void Cases<T>(string kind,int category,IEnumerable<T> values,List<object> cases)where T:class {
 var messages=values.ToArray();var generic=messages.Select(v=>SerializeWriter.ToBinary<T>(v)).ToArray();ClassInfo<T>.CategoryId=category;
 for(int n=0;n<messages.Length;n++)foreach(string mode in new[]{"unregistered-generic","registered-object","registered-typed"}){
 var typed=SerializeWriter.ToBinary<T>(messages[n]);var packet=mode=="unregistered-generic"?generic[n]:mode=="registered-object"?SerializeWriter.ToBinary((object)messages[n]):typed;
 T decoded;SerializeReader.FromBinary<T>(packet,out decoded);if(!Body(SerializeWriter.ToBinary<T>(decoded)).SequenceEqual(Body(typed)))throw new InvalidOperationException("Original roundtrip differs");object fields=null;
 if(decoded is CheckCharacterNameMessage){var m=(CheckCharacterNameMessage)(object)decoded;fields=new{name=m.Name,name_change=m.IsNameChange};}
 if(decoded is CheckCharacterNameResultMessage){var m=(CheckCharacterNameResultMessage)(object)decoded;fields=new{valid=m.Valid,name=m.Name,error=m.ErrorMsg};}
 if(decoded is ReservedInfoMessage){var m=(ReservedInfoMessage)(object)decoded;fields=new{names=m.ReservedName,titles=m.ReservedTitle};}
 if(decoded is _UpdatePlayState){fields=new{state=((_UpdatePlayState)(object)decoded).State};}
 cases.Add(new{kind,mode,category=mode=="registered-typed"?category:0,body=Body(packet),typed_body=Body(typed),is_null=decoded==null,fields});}
 }
 static void Main(){var cases=new List<object>();string[] names={null,"","LabFiona","lab_name","Lab\n","É-λ-😀","\0","long_synthetic_name_0123456789"};
 var check=new List<CheckCharacterNameMessage>{null};foreach(string n in names)foreach(bool change in new[]{false,true})check.Add(new CheckCharacterNameMessage{Name=n,IsNameChange=change});Cases("check",261,check,cases);
 var result=new List<CheckCharacterNameResultMessage>{null};foreach(string n in names)foreach(bool valid in new[]{false,true})foreach(string error in new[]{null,"","GameUI_Heroes_Error_Create_Character_InvalidName"})result.Add(new CheckCharacterNameResultMessage(valid,n,error));Cases("result",150,result,cases);
 Cases("query",439,new QueryReservedInfoMessage[]{null,new QueryReservedInfoMessage()},cases);
 var reserved=new List<ReservedInfoMessage>{null};foreach(string[] ns in new string[][]{null,new string[0],new string[]{null,"","LabFiona","É-λ-😀"}})foreach(int[] ts in new int[][]{null,new int[0],new int[]{int.MinValue,-1,0,1,int.MaxValue}})reserved.Add(new ReservedInfoMessage{ReservedName=ns,ReservedTitle=ts});Cases("reserved",443,reserved,cases);
 Cases("play",644,new _UpdatePlayState[]{null}.Concat(new[]{int.MinValue,-1,0,1,int.MaxValue}.Select(s=>new _UpdatePlayState{State=s})),cases);
 Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=4*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases}));}
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}