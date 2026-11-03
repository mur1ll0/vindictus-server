using System;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
class MailWireOracle {
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
    if(!Body(reencoded).SequenceEqual(Body(typed)))throw new InvalidOperationException("Roundtrip mismatch for "+kind+" case "+n);
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
 static MailItemInfo Item(long id,string cls,int count,bool received,bool expireable){
  return Obj<MailItemInfo>("MailItemID",id,"ItemID",id*7,"ItemClassEx",cls,"ItemCount",count,"Received",received,"Color1",-1,"Color2",int.MaxValue,"Color3",int.MinValue,"MaxDurabilityBonus",3,"ReducedDurability",-2,"IsCharacterBinded",!received,"IsExpireable",expireable,"ExpireDateTimeDiff",expireable?25920000000000L:-1L,"ExpireDateTimeTick",expireable?634647492000000000L:-1L);
 }
 static BriefMailInfo Brief(long id,byte type,string from,string title,int expire,byte attached,ICollection<MailItemInfo> items){
  return Obj<BriefMailInfo>("MailID",id,"MailType",type,"FromName",from,"ToName","Fiona","Title",title,"ExpireDate",expire,"IsRead",attached==2,"ItemAttached",attached,"ChargedGold",attached==1?500:0,"AttachedItemInfo",items);
 }
 // MailBox.SendMail fee term, the expression of the recovered source: the JIT's float
 // evaluation is recorded per architecture.
 static int FeeTerm(int price,int rate,int count){ return (int)((float)price*((float)rate/100f)*(float)count); }
 static void Main(){
  var cases=new List<object>();
  var full=new List<MailItemInfo>{Item(1,"gold",1500,false,false),null,Item(long.MaxValue,"sword[ENHANCE:9(0)<0>]",1,true,true)};
  AddCases("query_mail_list",530,new Guid("6f7864f6-5e49-33a3-a899-6a6801411695"),new[]{null,new QueryMailListMessage()},cases);
  AddCases("query_mail_info",531,new Guid("55a3b131-002a-3a3a-9e65-14132b97951d"),new[]{null,new QueryMailInfoMessage(0L),new QueryMailInfoMessage(long.MaxValue),new QueryMailInfoMessage(-1L)},cases);
  AddCases("get_mail_item",464,new Guid("5c2de52a-ae2c-3c6e-9060-14ca27b9eef3"),new[]{null,new GetMailItemMessage(638448480000016391L),new GetMailItemMessage(long.MinValue)},cases);
  AddCases("return_mail",561,new Guid("6eb38330-0409-37d3-9c8b-8062e9f45d48"),new[]{null,new ReturnMailMessage(5L),new ReturnMailMessage(long.MinValue)},cases);
  AddCases("delete_mail",452,new Guid("316f4412-0b62-3af6-9ea6-cf98aecce9cb"),new[]{null,new DeleteMailMessage(null),new DeleteMailMessage(new List<long>()),new DeleteMailMessage(new List<long>{1L,long.MaxValue,-5L})},cases);
  AddCases("send_mail",567,new Guid("fc081bf4-efe6-312b-9de9-b6c07f604817"),new[]{null,
   new SendMailMessage(1,"Evie","hello","text body",new List<AttachedTransferItemInfo>(),0,false,0),
   new SendMailMessage(3,null,null,null,null,int.MaxValue,true,int.MinValue),
   new SendMailMessage(2,"ペット","件名","内容",new List<AttachedTransferItemInfo>{Obj<AttachedTransferItemInfo>("ItemID",-1L,"ItemClass","fish_scale","ItemCount",20),null,Obj<AttachedTransferItemInfo>("ItemID",long.MaxValue,"ItemClass",null,"ItemCount",int.MinValue)},1500,false,0)},cases);
  AddCases("mail_list",495,new Guid("74393467-8b2a-3100-9eb1-b5a34ec3310b"),new[]{null,
   new MailListMessage(null,null,0),
   new MailListMessage(new List<BriefMailInfo>(),new List<BriefMailInfo>(),-1),
   new MailListMessage(new List<BriefMailInfo>{Brief(1L,9,"Heroes_GameUI_MailSender_SYSTEM","#GameUI_Heroes_GiveFailMail_Title,gold,10",604800,1,new List<MailItemInfo>{Item(2,"potion_hp_small",3,false,false)}),null,Brief(long.MaxValue,1,null,null,-1,0,null)},new List<BriefMailInfo>{Brief(-3L,255,"Evie","t",int.MinValue,2,full)},int.MaxValue)},cases);
  AddCases("mail_info",494,new Guid("720d93db-8a10-349d-9f31-81ae382857a6"),new[]{null,
   new MailInfoMessage(1L,2,"Evie","title","content",new List<MailItemInfo>(),0),
   new MailInfoMessage(long.MinValue,255,null,null,null,null,int.MaxValue),
   new MailInfoMessage(638448480000016391L,9,"Heroes_GameUI_MailSender_SYSTEM","GameUI_Heroes_Server_MailFactory_MakeReturnMail_Title,t","c",full,-1)},cases);
  AddCases("mail_sent",497,new Guid("06e86b5b-c00a-335e-a28f-92f6a517b156"),new[]{null,new MailSentMessage(MailSentMessage.ErrorCodeEnum.ReturnSuccessful),new MailSentMessage(MailSentMessage.ErrorCodeEnum.Successful),new MailSentMessage(MailSentMessage.ErrorCodeEnum.TransFerError_FeePayError),new MailSentMessage(MailSentMessage.ErrorCodeEnum.SelfCIDFailed)},cases);
  AddCases("mail_received",496,new Guid("67e4050b-e402-3a58-be39-fce090eb16dd"),new[]{null,new MailReceivedMessage("Evie",2),new MailReceivedMessage(null,255)},cases);
  AddCases("get_mail_item_completed",426,new Guid("203f4aaf-d073-3e30-8f1f-4d5458b4e338"),new[]{null,new GetMailItemCompletedMessage(1L,GetMailItemCompletedMessage.ResultEnum.Success),new GetMailItemCompletedMessage(long.MaxValue,GetMailItemCompletedMessage.ResultEnum.NoEmptySlot),new GetMailItemCompletedMessage(-1L,GetMailItemCompletedMessage.ResultEnum.Unknown)},cases);
  var fees=new List<object>();
  foreach(var f in new[]{new[]{1,2,1234},new[]{333,5,3},new[]{1,10,10},new[]{5000,0,7},new[]{7,3,1},new[]{99999,5,99},new[]{1,2,107374182},new[]{int.MaxValue,100,2},new[]{1,5,123456789}})
   fees.Add(new{price=f[0],rate=f[1],count=f[2],term=FeeTerm(f[0],f[1],f[2])});
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases,fee_cases=fees}));
 }
}
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
