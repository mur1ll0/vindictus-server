using System;
using System.Collections.Generic;
using System.Linq;
using System.Data.SqlTypes;
using System.IO;
using System.Web.Script.Serialization;
using ServiceCore.HeroesContents;
public class FeatureRow{public string key,value;}
public class RewardRow{public string item_utf16,feature;public int num,min_level,max_level;}
public class Package{public FeatureRow[] features;public RewardRow[] rewards;}
public class GetCharacters결과{public long ID;public int Level;public string Name;}
public class DormantUserEventSelectResult{public DateTime? IsDormantUserEvent;}
public class LoginService{}
public class QuietLog{public void ErrorFormat(string x,params object[] p){}public void WarnFormat(string x,params object[] p){}}
public static class Log<T>{public static QuietLog Logger=new QuietLog();}
public class Journal{public void AddJournal(string x){}}
public class User{public int ID=7;}
public static class LoginContents{public static List<DormantReward> DormantReward;}
public static class Case{
 public static Dictionary<string,string> Features;
 public static string CurrentVer{get{return Features["CurrentVer"];}}
 public static DateTime Now=new DateTime(1900,1,1).AddDays(100).AddTicks(16667);
}
public class Context{
 public DateTime? Latest,Dormant,Event;public int? Flag;
 public List<string> Actions=new List<string>();public List<object> Rewards=new List<object>();
 public GetCharacters결과[] Characters=new[]{new GetCharacters결과{ID=22,Level=20,Name=Program.Decode("00d80000")},new GetCharacters결과{ID=11,Level=60,Name=null},new GetCharacters결과{ID=33,Level=int.MinValue,Name="synthetic"}};
 public void GetLatestConnectedDate(int uid,ref DateTime? result){result=Latest;}
 public void GetDormantUserData(int uid,ref DateTime? result){result=Dormant;}
 public void SetFriendRecommendFlag(int uid,int flag){Actions.Add("friend");Flag=flag;}
 public void ReleaseDormant(int uid){Actions.Add("release");Dormant=null;}
 public void SetDormantUser(int uid,DateTime date){Actions.Add("set");Dormant=new SqlDateTime(date).Value;}
 public IEnumerable<GetCharacters결과> GetCharacters(int uid){Actions.Add("characters");return Characters;}
 public void SendDormantItem(long cid,string item,int num,bool bound,string title,string content){
  long clock=2592000000L+Rewards.Count;
  Actions.Add("reward");Rewards.Add(new{cid=cid.ToString(),item=Program.Hex(item),count=num,bound=bound,title=Program.Hex(title),content=Program.Hex(content),clock=clock.ToString()});
 }
 public void DormantUserEventUpdate(int uid,DateTime? date){Actions.Add(date.HasValue?"event_set":"event_clear");Event=date.HasValue?(DateTime?)new SqlDateTime(date.Value).Value:null;}
 public IEnumerable<DormantUserEventSelectResult> DormantUserEventSelect(int uid){Actions.Add("event_read");return new[]{new DormantUserEventSelectResult{IsDormantUserEvent=Event}};}
}
public static class Program{
 public static string Decode(string hex){if(hex==null)return null;var chars=new char[hex.Length/4];for(int i=0;i<chars.Length;i++)chars[i]=(char)(Convert.ToByte(hex.Substring(i*4,2),16)|(Convert.ToByte(hex.Substring(i*4+2,2),16)<<8));return new string(chars);}
 public static string Hex(string s){if(s==null)return null;var b=new byte[s.Length*2];for(int i=0;i<s.Length;i++){b[2*i]=(byte)s[i];b[2*i+1]=(byte)(s[i]>>8);}return BitConverter.ToString(b).Replace("-","").ToLowerInvariant();}
 public static string Units(DateTime? date){if(!date.HasValue)return null;var d=new SqlDateTime(date.Value);return ((long)d.DayTicks*25920000+d.TimeTicks).ToString();}
 public static DateTime? Days(int? value){return value.HasValue?(DateTime?)new DateTime(1900,1,1).AddDays(value.Value):null;}
 public static void Main(string[] args){
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var p=new JavaScriptSerializer().Deserialize<Package>(File.ReadAllText(args[0]));
  LoginContents.DormantReward=p.rewards.Select(x=>new DormantReward{RewardItemClass=Decode(x.item_utf16),Num=x.num,MinLevel=x.min_level,MaxLevel=x.max_level,Feature=x.feature}).ToList();
  var cases=new List<object>();
  for(int friend=0;friend<2;friend++)for(int dormant=0;dormant<2;dormant++)for(int v2=0;v2<2;v2++)for(int scenario=0;scenario<6;scenario++){
   Case.Features=p.features.ToDictionary(x=>x.key,x=>x.value??"");
   Case.Features["friend_loyalty_system"]=friend==1?"1.09":"";Case.Features["friend_loyalty_return_interval"]="7";
   Case.Features["DormantAccount"]=dormant==1?"1.09":"";Case.Features["DormantAccountInterval"]="7";Case.Features["DormantNewAccountInterval"]="1";Case.Features["DormantAccount_ver2"]=v2==1?"1.09":"";
   var c=new Context{Latest=Days(scenario==0?(int?)null:scenario==1?70:scenario==4?110:90),Dormant=Days(scenario==2?98:scenario==3?50:scenario==5?(int?)100:null),Event=Days(scenario>=2?(int?)40:null)};
   if(scenario==5)c.Flag=0;
   var input=new{latest=Units(c.Latest),since=Units(c.Dormant),event_date=Units(c.Event),friend_flag=c.Flag};
   var flags=RecoveredLogin.Run(c);
   cases.Add(new{friend=friend==1,dormant=dormant==1,v2=v2==1,scenario=scenario,input=input,now_ticks=Case.Now.Ticks.ToString(),expected=new{flags=flags,friend_flag=c.Flag,since=Units(c.Dormant),event_date=Units(c.Event),rewards=c.Rewards,actions=c.Actions}});
  }
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=4*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases=cases}));
 }
}
