using System;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class PetWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v is DateTime)return new {ticks=((DateTime)v).Ticks,kind=(int)((DateTime)v).Kind};
  if(v.GetType().IsEnum)return Convert.ToInt32(v);
  if(v is string || v.GetType().IsPrimitive)return v;
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  return v.GetType().GetProperties().Where(p=>p.CanRead).ToDictionary(p=>p.Name,p=>Capture(p.GetValue(v,null)));
 }
 static T Obj<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetProperty((string)fields[i]).SetValue(obj,fields[i+1],null);
  return obj;
 }
 static void AddCases<T>(string kind,int category,Guid expectedGuid,IEnumerable<T> values,List<object> cases) where T:class {
  if(typeof(T).GUID!=expectedGuid)throw new InvalidOperationException("GUID mismatch for "+typeof(T).FullName);
  var items=values.ToArray();
  var generic=items.Select(v=>SerializeWriter.ToBinary<T>(v)).ToArray();
  ClassInfo<T>.CategoryId=category;
  for(int n=0;n<items.Length;n++){
   var typed=SerializeWriter.ToBinary<T>(items[n]);
   foreach(string mode in new[]{"unregistered-generic","registered-object","registered-typed"}){
    var packet=mode=="unregistered-generic"?generic[n]:mode=="registered-object"?SerializeWriter.ToBinary((object)items[n]):typed;
    T decoded;
    SerializeReader.FromBinary<T>(packet,out decoded);
    var reencoded=SerializeWriter.ToBinary<T>(decoded);
    if(!Body(reencoded).SequenceEqual(Body(typed))){var a=Body(reencoded);var b=Body(typed);int d=0;while(d<a.Length&&d<b.Length&&a[d]==b[d])d++;throw new InvalidOperationException("Roundtrip mismatch for "+kind+" case "+n+" at "+d+" of "+a.Length+"/"+b.Length+": "+string.Join(",",b.Skip(Math.Max(0,d-12)).Take(30))+" vs "+string.Join(",",a.Skip(Math.Max(0,d-12)).Take(30)));}
    var analyzer=new MessageAnalyzer();
    var raw=packet.Bytes;
    var copy=new byte[raw.Count];
    Buffer.BlockCopy(raw.Array,raw.Offset,copy,0,raw.Count);
    analyzer.Add(new ArraySegment<byte>(copy));
    var analyzed=analyzer.ToArray();
    if(analyzed.Length!=1)throw new InvalidOperationException("MessageAnalyzer count mismatch");
    var framed=new Packet(analyzed[0]);
    if(framed.CategoryId!=packet.CategoryId||!Body(framed).SequenceEqual(Body(packet)))throw new InvalidOperationException("MessageAnalyzer payload mismatch");
    cases.Add(new{kind,mode,category=packet.CategoryId,body=Body(packet),typed_body=Body(typed),is_null=decoded==null,fields=Capture(decoded)});
   }
  }
 }
 // A complete pet as MakePetStatusInfo builds it: default skills without expiry, a cash
 // skill with ExpireDateTimeDiff, an accessory, and a fixed UTC RemainExpiredTime.
 static PetStatusInfo Pet(long id,string name,int seed){
  var p=new PetStatusInfo();
  p.PetID=id;p.PetName=name;p.PetType=518+seed;p.Slot=seed;p.Level=30;p.Exp=int.MaxValue;p.Desire=200;
  p.Stat=new PetStatElement(88,88,624,25,2,70,80,740,152,2,int.MinValue);
  p.Skills=new List<PetSkillElement>{Obj<PetSkillElement>("SkillID",6,"SlotOrder",0,"OpenLevel",1,"HasExpireDateTimeInfo",false,"ExpireDateTimeDiff",-1L),Obj<PetSkillElement>("SkillID",9,"SlotOrder",101,"OpenLevel",1,"HasExpireDateTimeInfo",true,"ExpireDateTimeDiff",25920000000000L)};
  p.Accessories=new List<PetAccessoryElement>{new PetAccessoryElement("pet_accessory_test",1,2,-3)};
  p.PetStatus=4;p.RemainActiveTime=86400;
  p.RemainExpiredTime=new DateTime(634647492000000000L,DateTimeKind.Utc);
  return p;
 }
 static void Main(){
  var cases=new List<object>();
  var empty=new PetStatusInfo();
  var bare=Obj<PetStatusInfo>("PetName","","PetType",0);
  var early=Pet(1,"Rex",1);early.RemainExpiredTime=new DateTime(0L,DateTimeKind.Utc);
  var late=Pet(2,"Fido",2);late.RemainExpiredTime=new DateTime(3155378975999999999L,DateTimeKind.Utc);
  AddCases("pet_list",324,new Guid("62804a1d-30eb-310c-8792-0b6aa839ffd3"),new[]{null,new PetListMessage(null,true),new PetListMessage(new List<PetStatusInfo>(),false),new PetListMessage(new List<PetStatusInfo>{empty,null,bare},true),new PetListMessage(new List<PetStatusInfo>{Pet(long.MaxValue,"Bolt",0),early,late},false)},cases);
  AddCases("update_current_pet",323,new Guid("9fddbc1b-3c9e-3c79-8b50-457c8379c699"),new[]{null,new UpdateCurrentPetMessage(0,null),new UpdateCurrentPetMessage(-1,bare),new UpdateCurrentPetMessage(int.MaxValue,Pet(long.MinValue,"Gremlin",3))},cases);
  AddCases("pet_feed_list",325,new Guid("2fcaaf5c-a077-375d-933c-92d298581487"),new[]{null,new PetFeedListMessage(null,false),new PetFeedListMessage(new List<PetFeedElement>(),true),new PetFeedListMessage(new List<PetFeedElement>{new PetFeedElement(1,25),null,new PetFeedElement(255,int.MinValue)},false)},cases);
  AddCases("query_pet_list",316,new Guid("e94af65d-e66f-31fd-b8d0-53a042370b5c"),new[]{null,new QueryPetListMessage()},cases);
  AddCases("select_pet",315,new Guid("5736fde9-bc21-3b82-a84e-72bb0a517a14"),new[]{null,Obj<SelectPetMessage>("PetID",0L),Obj<SelectPetMessage>("PetID",long.MaxValue),Obj<SelectPetMessage>("PetID",-1L)},cases);
  AddCases("move_pet_slot",313,new Guid("86698206-912c-36be-9cc1-257a396935fb"),new[]{null,Obj<MovePetSlotMessage>("PetID",5L,"DestSlotNo",99),Obj<MovePetSlotMessage>("PetID",long.MinValue,"DestSlotNo",int.MinValue)},cases);
  AddCases("remove_pet",314,new Guid("9f8b0d6d-ae8e-388f-8841-11aa2aa37a0d"),new[]{null,Obj<RemovePetMessage>("PetID",1L),Obj<RemovePetMessage>("PetID",long.MinValue)},cases);
  AddCases("pet_revived",317,new Guid("e145a882-d89c-3494-97bf-607b1aeaaab5"),new[]{null,Obj<PetRevivedMessage>("CasterTag",1,"ReviverTag",-1,"PetID",long.MaxValue,"Method","pet_resurrection_solo"),Obj<PetRevivedMessage>("CasterTag",int.MinValue,"ReviverTag",int.MaxValue,"PetID",0L,"Method",null)},cases);
  AddCases("pet_killed",318,new Guid("3079bc3a-9b67-3de0-a504-87a66664647f"),new[]{null,new PetKilledMessage(0,1L),new PetKilledMessage(int.MinValue,long.MaxValue)},cases);
  AddCases("remove_pet_skill",319,new Guid("8aaf9011-eb3a-3fc1-a40b-25602676547b"),new[]{null,Obj<RemovePetSkillMessage>("PetID",7L,"PetSkillID",9),Obj<RemovePetSkillMessage>("PetID",long.MinValue,"PetSkillID",int.MaxValue)},cases);
  AddCases("pet_create_name_check",320,new Guid("efd95e57-04fa-3e93-9271-9bcc29422fb0"),new[]{null,Obj<PetCreateNameCheckMessage>("ItemID",3L,"PetName","Rex"),Obj<PetCreateNameCheckMessage>("ItemID",long.MinValue,"PetName",null),Obj<PetCreateNameCheckMessage>("ItemID",0L,"PetName","ペット")},cases);
  AddCases("pet_create_name_check_result",321,new Guid("133b280b-47e4-34cc-b16a-a8a03eb4dc97"),new[]{null,new PetCreateNameCheckResultMessage(3L,"Rex",true),new PetCreateNameCheckResultMessage(long.MaxValue,null,false)},cases);
  AddCases("pet_create",322,new Guid("942945c3-e95f-3bb4-9be2-a7f984434f41"),new[]{null,Obj<PetCreateMessage>("ItemID",3L,"PetName","Rex"),Obj<PetCreateMessage>("ItemID",long.MinValue,"PetName",null)},cases);
  AddCases("request_pet_food_share",326,new Guid("0d488183-a8d0-3730-99ab-1088836ab2d4"),new[]{null,new RequestPetFoodShareMessage(9L),new RequestPetFoodShareMessage(long.MinValue)},cases);
  AddCases("request_pet_food_unshare",327,new Guid("5922ce58-625f-3c91-8b08-7331f63b5a27"),new[]{null,new RequestPetFoodUnshareMessage("pet_feed_make"),new RequestPetFoodUnshareMessage(null)},cases);
  AddCases("pet_change_name_check",328,new Guid("ad77125f-4472-336f-a50e-45d79ec35a0e"),new[]{null,Obj<PetChangeNameCheckMessage>("ItemID",1L,"PetID",2L,"PetName","Rex"),Obj<PetChangeNameCheckMessage>("ItemID",long.MinValue,"PetID",long.MaxValue,"PetName",null)},cases);
  AddCases("pet_change_name_check_result",329,new Guid("8b057824-a031-3410-a24b-f52be6c03cf3"),new[]{null,new PetChangeNameCheckResultMessage(true,1L,2L,"Rex","Success"),new PetChangeNameCheckResultMessage(false,long.MinValue,long.MaxValue,null,null)},cases);
  AddCases("pet_change_name",330,new Guid("041d653f-01e7-371a-b86a-1346b110d36c"),new[]{null,Obj<PetChangeNameMessage>("ItemID",1L,"PetID",2L,"PetName","Rex"),Obj<PetChangeNameMessage>("ItemID",0L,"PetID",-1L,"PetName",null)},cases);
  AddCases("pet_change_name_result",331,new Guid("a9c12ba2-27cd-3864-9786-42d4f295dff5"),new[]{null,new PetChangeNameResultMessage(true,"Rex","Success"),new PetChangeNameResultMessage(false,null,"ChangeName_HasNotPet")},cases);
  AddCases("pet_operation",332,new Guid("bf092479-3775-3d89-8f77-53bcc8c681f4"),new[]{null,Obj<PetOperationMessage>("OperationCode",PetOperationType._CHEAT_AddPet,"PetID",0L,"Arg","Rex","Value1",518,"Value2",0,"Value3",0),Obj<PetOperationMessage>("OperationCode",PetOperationType._CHEAT_ChangePetSlot,"PetID",long.MaxValue,"Arg",null,"Value1",int.MinValue,"Value2",-1,"Value3",int.MaxValue)},cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
