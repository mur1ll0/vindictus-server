using System;
using System.Linq;
using System.Collections.Generic;
using System.Text;
using System.Net;
using System.Web.Script.Serialization;
using ServiceCore.Configuration;
using ServiceCore.EndPointNetwork;
using Utility;
using UserEntity = Recovered;
static class Clock { public static DateTime Now=new DateTime(638000000000000000,DateTimeKind.Utc); }
class User {public string Name="MiXeDLab";public string SecondPassword;public int? SecondPasswordFailCount;public DateTime? SecondPasswordLastFail;}
class UserDBDataContext:IDisposable {public static User Row;public IQueryable<User> User {get{return new[]{Row}.AsQueryable();}}public void SubmitChanges(){}public int SecondPasswordLimitCheck(string ip){return 1;}public void Dispose(){}}
static class FeatureMatrix {public static int Mode;public static int ServerCode=1;public static bool IsEnable(string k){return k=="SecondPassword";}public static int GetInteger(string k){return Mode;}}
class Logger {public void Error(object m){}public void Error(string m,object e){}}
static class Log<T>{public static Logger Logger=new Logger();}
class Recovered {
 public string NexonID="MiXeDLab";public IPAddress LoginAddr=IPAddress.Loopback;
 private DateTime? secondPasswordRetryRestrictedUntil;
 private static Func<string,string,string>[] hashFunc;
 private static Func<string,string,string,bool>[] hashCompareFunc;
 static Recovered(){/*HASH_ARRAYS*/}
 /*METHODS*/
 public static int Version(string s){return new Recovered().GetHashVersion(s);}
 public static string Hash(string n,string p,int mode){return hashFunc[mode](n,p);}
 public long? Until {get{return secondPasswordRetryRestrictedUntil.HasValue?(long?)secondPasswordRetryRestrictedUntil.Value.Ticks:null;}}
}
static class Program {
 static int[] Units(string s){return s==null?null:s.Select(c=>(int)c).ToArray();}
 static string Password(int[] u){return new string(u.Select(v=>(char)v).ToArray());}
 static object Row(){var u=UserDBDataContext.Row;return new{password=Units(u.SecondPassword),count=u.SecondPasswordFailCount,last=u.SecondPasswordLastFail.HasValue?(long?)u.SecondPasswordLastFail.Value.Ticks:null};}
 static void Main(){
  var hashes=new List<object>();var queries=new List<object>();var checks=new List<object>();var registers=new List<object>();var versions=new List<object>();
  int[][] passwords={new int[0],new[]{49,50,51,52,53,54},new[]{65,0,66},new[]{233},new[]{55357,56832},new[]{55296},Enumerable.Repeat(65,256).ToArray()};
  foreach(string name in new[]{"MiXeDLab","ILab","lab"})foreach(int mode in new[]{0,1,2})foreach(var pw in passwords){hashes.Add(new{name,mode,password=pw,encoded=Units(Recovered.Hash(name,Password(pw),mode))});}
  foreach(int length in new[]{0,1,127,128,129,130,131,256})versions.Add(new{length,version=Recovered.Version(new string('a',length))});
  foreach(int passwordKind in new[]{-1,0,1})foreach(int? count in new int?[]{null,-1,0,4,5,6,2147483647})foreach(long? last in new long?[]{null,552877920000000000,Clock.Now.Ticks-6100000000,Clock.Now.Ticks-6000000000,Clock.Now.Ticks-5990000000,Clock.Now.Ticks}){
   string stored=passwordKind==-1?null:passwordKind==0?"":"hash";
   UserDBDataContext.Row=new User{SecondPassword=stored,SecondPasswordFailCount=count,SecondPasswordLastFail=last.HasValue?(DateTime?)new DateTime(last.Value):null};
   var r=new Recovered();HasSecondPasswordMessage m;r.QuerySecondPassword(out m);
   queries.Add(new{stored=Units(stored),count,last,first=m.IsFirstQuery,has=m.HasSecondPassword,fail=m.FailCount,retry=m.RetryLockedSec,until=r.Until,after=Row()});
  }
  foreach(int mode in new[]{0,1,2})foreach(int oldMode in new[]{0,1,2})foreach(bool correct in new[]{false,true})foreach(int? count in new int?[]{null,0,4,5,6}){
   FeatureMatrix.Mode=mode;string stored=Recovered.Hash("MiXeDLab","123456",oldMode);UserDBDataContext.Row=new User{SecondPassword=stored,SecondPasswordFailCount=count};
   var r=new Recovered();SecondPasswordResultMessage m;r.CheckSecondPassword(correct?"123456":"654321",out m);
   checks.Add(new{mode,oldMode,correct,count,stored=Units(stored),passed=m.Passed,fail=m.FailCount,retry=m.RetryLockedSec,until=r.Until,after=Row()});
  }
  foreach(int mode in new[]{0,1,2})foreach(var pw in passwords){FeatureMatrix.Mode=mode;UserDBDataContext.Row=new User{SecondPassword="old",SecondPasswordFailCount=4,SecondPasswordLastFail=Clock.Now};var r=new Recovered();SecondPasswordResultMessage m;r.RegisterSecondPassword(Password(pw),out m);registers.Add(new{mode,password=pw,type=(int)m.OperationType,passed=m.Passed,fail=m.FailCount,retry=m.RetryLockedSec,after=Row()});}
  Console.OutputEncoding=new UTF8Encoding(false);Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=10000000}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",now=Clock.Now.Ticks,hashes,versions,queries,checks,registers}));
 }
}
