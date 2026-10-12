using System;
using System.Collections.Generic;
using System.Data.SqlTypes;
using System.Linq;
using System.Web.Script.Serialization;
public enum MailType { GMMail, GuildLevelUpItemMail, AuctionItemMail, DungeonItemMail }
public class QueuedItem { public long CID=7; public bool Processed; public string EventCode; public DateTime? ExpireTime; public long? ItemID; public string MailTitle,MailContent; }
public class Context { public List<QueuedItem> QueuedItem=new List<QueuedItem>(); }
public class ItemOwner { public long CID=7; public Loader Loader=new Loader(); }
public class ItemClassInfo { public string ItemClass,Bind; }
public class ItemAttributeElement {}
public static class ItemGenerator { public static bool CheckCharacterBind(bool b,ItemClassInfo i,IDictionary<string,ItemAttributeElement> a){throw new NotSupportedException();} }
public static class ItemFestivalProcessor { public static HashSet<long> RefreshTarget=new HashSet<long>(); }
namespace ServiceCore { public static class FeatureMatrix { public static bool IsEnable(string s){Case.FeatureCalls++;if(Case.ThrowFeature)throw new InvalidOperationException();return Case.Feature;} } }
public class Logger { public void ErrorFormat(string format,params object[] args){Case.Errors++;} }
public static class Log<T> { public static Logger Logger=new Logger(); }
public class QueuedItemController {}
public static class Case {
 public static DateTime Now;public static bool ThrowRow,Feature,ThrowFeature;
 public static int Attempted,Errors,FeatureCalls;
}
public static class Program {
 static string Hex(string value) {
  if(value==null)return null;
  return String.Concat(value.Select(c=>((byte)c).ToString("x2")+((byte)(c>>8)).ToString("x2")));
 }
 public static void Main() {
  var events=new string[]{null,"","GuildLevelUp","MobileAppBuy","FriendEventReward_Add","guildlevelup","GuildLevelUp\0","unmatched","\ud800"};
  var attempts=new List<object>();var routes=new List<object>();var binds=new List<object>();var selection=new List<object>();
  foreach(var e in events)for(int f=0;f<2;f++)for(int r=0;r<2;r++)for(int t=0;t<2;t++) {
   var owner=new ItemOwner();owner.Loader.Context.QueuedItem.Add(new QueuedItem{EventCode=e});
   ItemFestivalProcessor.RefreshTarget.Clear();if(r==1)ItemFestivalProcessor.RefreshTarget.Add(owner.CID);
   Case.ThrowRow=t==1;Case.Attempted=0;Case.Errors=0;
   int count=Recovered.ProcessQueuedItem(0,owner,f==1);
   attempts.Add(new{event_hex=Hex(e),festival=f==1,refresh=r==1,throws=t==1,attempted=Case.Attempted,event_count=count,errors=Case.Errors});
  }
  long epoch=599266080000000000;
  long?[] units={null,0,1,1,-1,76683386879999,-1391644800000};
  long[] clocks={epoch,epoch,epoch+29999,epoch+30000,epoch,3155378975999999999,0};
  foreach(var e in events)for(int id=0;id<2;id++)for(int m=0;m<4;m++)for(int x=0;x<units.Length;x++){
   DateTime? expiry=null;
   if(units[x].HasValue){
    long u=units[x].Value;long days=u/25920000;long rest=u%25920000;
    if(rest<0){days--;rest+=25920000;}
    expiry=new SqlDateTime(new DateTime(epoch).AddDays(days).AddMilliseconds((rest*10+1)/3)).Value;
   }
   Case.Now=new DateTime(clocks[x]);
   var row=new QueuedItem{EventCode=e,ItemID=id==0?(long?)null:-1,MailTitle=(m&1)==0?null:"",MailContent=(m&2)==0?null:"\ud800",ExpireTime=expiry};
   routes.Add(new{event_hex=Hex(e),item_id=row.ItemID,title_hex=Hex(row.MailTitle),content_hex=Hex(row.MailContent),expire_units=units[x].HasValue?units[x].Value.ToString():null,now_ticks=clocks[x].ToString(),expected=Recovered.Route(row)});
  }
  foreach(var c in new string[]{"gold","silver_coin","Gold","gold\0","item","\ud800",""})for(int b=0;b<2;b++)for(int f=0;f<2;f++)for(int t=0;t<2;t++){
   Case.Feature=f==1;Case.ThrowFeature=t==1;Case.FeatureCalls=0;bool? result=null;bool error=false;
   try{result=Recovered.Bind(b==1,c);}catch(InvalidOperationException){error=true;}
   binds.Add(new{item_hex=Hex(c),bound=b==1,feature=f==1,throws=t==1,result,error,feature_calls=Case.FeatureCalls});
  }
  foreach(var cid in new long[]{7,-1,long.MinValue,long.MaxValue})for(int p=0;p<2;p++){
   var loader=new Loader();loader.Context.QueuedItem.Add(new QueuedItem{CID=cid,Processed=p==1});
   selection.Add(new{cid=cid.ToString(),processed=p==1,selected=loader.GetQueuedItems().Count()==1});
  }
  Console.Write(new JavaScriptSerializer{MaxJsonLength=2*1024*1024}.Serialize(new{architecture=IntPtr.Size==8?"x64":"x86",attempts,routes,binds,selection}));
 }
}
