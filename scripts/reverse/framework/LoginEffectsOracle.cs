using System;
using System.Collections.Generic;
using System.Linq;
using System.Web.Script.Serialization;
using System.Data.Linq;
using ServiceCore.HeroesContents;
namespace System.Data.Linq { public interface ISingleResult<T>:IEnumerable<T> {} }
namespace System.Data.Linq.Mapping {
 [AttributeUsage(AttributeTargets.Class)] public sealed class TableAttribute:Attribute { public string Name{get;set;} }
 [AttributeUsage(AttributeTargets.Property)] public sealed class ColumnAttribute:Attribute { public string Storage{get;set;} public string DbType{get;set;} public bool CanBeNull{get;set;} }
}
public sealed class Rows<T>:List<T>,ISingleResult<T> {}
public sealed class GetCharacters결과 { public int Level=42; public long ID=1; public string Name="Synthetic"; }
public sealed class DormantUserEventSelectResult { public DateTime? IsDormantUserEvent; }
public sealed class LoginService {}
public static class Log<T> { public static Logger Logger=new Logger(); }
public sealed class Logger { public void WarnFormat(string text,params object[] values){} }
public sealed class UserEntity { public void AddJournal(string value){} }
public sealed class User { public int ID=1; }
public static class LoginContents { public static List<DormantReward> DormantReward=new List<DormantReward>(); }
namespace ServiceCore {
 public static class FeatureMatrix {
  public static bool IsEnable(string key) {
   if(key=="friend_loyalty_system")return Case.Current.friend_loyalty;
   if(key=="DormantAccount")return Case.Current.dormant;
   if(key=="DormantAccount_ver2")return Case.Current.v2;
   if(key=="synthetic_reward")return RewardEnabled;
   throw new InvalidOperationException("Unknown feature");
  }
  public static bool RewardEnabled;
  public static int GetInteger(string key) {
   if(key=="friend_loyalty_return_interval")return Case.Current.friend_days;
   if(key=="DormantNewAccountInterval")return Case.Current.new_days;
   if(key=="DormantAccountInterval")return Case.Current.dormant_days;
   throw new InvalidOperationException("Unknown integer feature");
  }
 }
}
public static class Clock {
 public static DateTime Loyalty {get{return Case.Date(Case.Current.loyalty_now);}}
 public static DateTime DormantElapsed {get{return Case.Date(Case.Current.dormant_now);}}
 public static DateTime LatestElapsed {get{return Case.Date(Case.Current.latest_now);}}
 public static DateTime SetDormant {get{return Case.Date(Case.Current.set_now);}}
 public static DateTime SetEvent {get{return Case.Date(Case.Current.event_now);}}
}
public sealed class UserTable {
 private Context context;
 public UserTable(Context context){this.context=context;}
 public void InsertOnSubmit(User user){context.Actions.Add("create_user");}
}
public sealed class Context {
 public UserTable User;
 public Context(){User=new UserTable(this);}
 public void SubmitChanges(){}
 public List<string> Actions=new List<string>();
 private int latestCalls;
 public void GetLatestConnectedDate(int uid,ref DateTime? latest) {
  bool loyaltyFirst=Case.Current.friend_loyalty&&!Case.Current.flag.HasValue
   && !(Case.Current.new_user&&!Case.Current.flag.HasValue);
  latest=Case.NullableDate(loyaltyFirst&&latestCalls++==0?Case.Current.latest_loyalty:Case.Current.latest_dormant);
 }
 public void GetDormantUserData(int uid,ref DateTime? data){data=Case.NullableDate(Case.Current.since);}
 public void SetFriendRecommendFlag(int uid,int flag){Actions.Add("friend:"+flag);}
 public void ReplaceUserAchievementNIDtoUID(string nid,int uid){Actions.Add("achievement");}
 public void ReleaseDormant(int uid){Actions.Add("release");}
 public void SetDormantUser(int uid,DateTime date){Actions.Add("dormant:"+date.Ticks);}
 public ISingleResult<GetCharacters결과> GetCharacters(int uid){Actions.Add("rewards");return new Rows<GetCharacters결과>();}
 public void SendDormantItem(long cid,string item,int count,bool bound,string title,string body){throw new InvalidOperationException("Queue executor not emulated");}
 public void DormantUserEventUpdate(int uid,DateTime? date){Actions.Add("event:"+(date.HasValue?date.Value.Ticks.ToString():"null"));}
 public ISingleResult<DormantUserEventSelectResult> DormantUserEventSelect(int uid) {
  Actions.Add("read_event");
  if(Case.Current.events==null)return null;
  var rows=new Rows<DormantUserEventSelectResult>();
  foreach(string value in Case.Current.events)rows.Add(new DormantUserEventSelectResult{IsDormantUserEvent=Case.NullableDate(value)});
  return rows;
 }
}
public sealed class Case {
 public static Case Current;
 public string label;
 public bool new_user,friend_loyalty,dormant,v2;
 public int? flag;
 public int friend_days,new_days,dormant_days;
 public string latest_loyalty,since,latest_dormant,loyalty_now,dormant_now,latest_now,set_now,event_now;
 public string[] events;
 public static DateTime Date(string ticks){return new DateTime(long.Parse(ticks),DateTimeKind.Utc);}
 public static DateTime? NullableDate(string ticks){return ticks==null?(DateTime?)null:Date(ticks);}
}
public static class LoginEffectsOracle {
 static Case Make(string label) {
  string now=new DateTime(2020,1,1,0,0,0,DateTimeKind.Utc).Ticks.ToString();
  return new Case{label=label,loyalty_now=now,dormant_now=now,latest_now=now,set_now=now,event_now=(long.Parse(now)+1234567).ToString(),events=new string[]{null}};
 }
 public static void Main() {
  const long day=864000000000;
  long?[] ages={null,-day,-day+1,day-1,day,30*day-1,30*day,30*day+1};
  int[] thresholds={-1,0,1,30,int.MaxValue};
  var cases=new List<object>();
  int n=0;
  for(int mask=0;mask<8;mask++)for(int a=0;a<ages.Length;a++)foreach(int threshold in thresholds) {
   Case c=Make("matrix_"+n);
   c.new_user=n%4==0;c.friend_loyalty=(mask&1)!=0;c.dormant=(mask&2)!=0;c.v2=(mask&4)!=0;
   c.flag=n%3==0?(int?)null:(n%3==1?0:2);
   c.friend_days=threshold;c.new_days=threshold;c.dormant_days=threshold;
   c.latest_loyalty=ages[a].HasValue?(long.Parse(c.loyalty_now)-ages[a].Value).ToString():null;
   c.since=n%3==0?null:c.latest_loyalty;
   c.latest_dormant=n%5==0?null:c.latest_loyalty;
   c.events=n%3==0?null:(n%3==1?new string[]{null}:new string[]{null,c.set_now});
   Case.Current=c;cases.Add(new{input=c,output=RecoveredEffects.Run()});n++;
  }
  foreach(long future in new[]{-day-1,-day,-day+1,0L,day-1,day,day+1}) {
   Case c=Make("signed_day_"+future);c.friend_loyalty=true;c.dormant=true;c.v2=true;
   c.friend_days=-1;c.new_days=-1;c.dormant_days=-1;
   c.since=(long.Parse(c.dormant_now)-future).ToString();c.latest_loyalty=c.since;c.latest_dormant=c.since;
   Case.Current=c;cases.Add(new{input=c,output=RecoveredEffects.Run()});
  }
  Case moving=Make("separate_clock_samples");
  moving.friend_loyalty=true;moving.dormant=true;moving.v2=true;moving.friend_days=1;moving.new_days=1;moving.dormant_days=1;
  moving.since=(long.Parse(moving.dormant_now)-day).ToString();moving.latest_loyalty=moving.since;moving.latest_dormant=moving.since;
  moving.loyalty_now=(long.Parse(moving.loyalty_now)-1).ToString();moving.latest_now=(long.Parse(moving.latest_now)+1).ToString();
  Case.Current=moving;cases.Add(new{input=moving,output=RecoveredEffects.Run()});
  foreach(bool reverse in new[]{false,true}) {
   Case c=Make(reverse?"date_range_reverse":"date_range_forward");
   c.friend_loyalty=true;c.dormant=true;c.v2=true;c.friend_days=int.MinValue;c.new_days=int.MinValue;c.dormant_days=int.MinValue;
   c.loyalty_now=c.dormant_now=c.latest_now=reverse?"0":DateTime.MaxValue.Ticks.ToString();
   c.latest_loyalty=c.since=c.latest_dormant=reverse?DateTime.MaxValue.Ticks.ToString():"0";
   Case.Current=c;cases.Add(new{input=c,output=RecoveredEffects.Run()});
  }
  var rewards=new List<object>();
  int[] values={int.MinValue,-1,0,1,42,100,int.MaxValue};
  foreach(int level in values)foreach(int min in values)foreach(int max in values)foreach(bool enabled in new[]{false,true}) {
   ServiceCore.FeatureMatrix.RewardEnabled=enabled;
   rewards.Add(new{level,min,max,enabled,matched=RecoveredEffects.Reward(level,min,max)});
  }
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=16*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases,rewards}));
 }
}

