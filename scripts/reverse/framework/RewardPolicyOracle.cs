using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Web.Script.Serialization;
using System.Data.SqlTypes;
using System.Data.SqlClient;
using System.Security.Cryptography;
using ServiceCore.HeroesContents;
public class FeatureRow { public string key;public string value; }
public class RewardRow { public string item_utf16;public int num,min_level,max_level;public string feature; }
public class Manifest { public string region;public int game_code,server_code,catalog_rows; }
public class Package { public Manifest manifest;public FeatureRow[] features;public RewardRow[] rewards; }
public class CharacterRow { public int Level;public long ID;public string Name; }
public class GetCharacters결과 : CharacterRow { }
public static class LoginContents { public static List<DormantReward> DormantReward; }
public static class Case { public static Dictionary<string,string> Features; public static string CurrentVer; }
public class QuietLog { public void ErrorFormat(string f,params object[] p){} }
public static class Log<T> { public static QuietLog Logger=new QuietLog(); }
public class Context {
 public List<byte[]> Calls=new List<byte[]>();
 public void SendDormantItem(long cid,string item,int num,bool bound,string title,string content) {
  using(var stream=new MemoryStream())using(var w=new BinaryWriter(stream)) {
   w.Write(System.Text.Encoding.ASCII.GetBytes("SendDormantItem\0v1"));w.Write(cid);w.Write(num);w.Write((byte)(bound?1:0));w.Write((long)0);
   foreach(var field in new[]{item,title,content}) { if(field==null){w.Write((byte)0);continue;}w.Write((byte)1);var data=Program.Unicode(field);w.Write(data.Length);w.Write(data); }
   Calls.Add(stream.ToArray());
  }
 }
}
public static class Program {
 public static byte[] Unicode(string s){var b=new byte[s.Length*2];for(int i=0;i<s.Length;i++){b[2*i]=(byte)s[i];b[2*i+1]=(byte)(s[i]>>8);}return b;}
 public static string Decode(string hex){if(hex==null)return null;var chars=new char[hex.Length/4];for(int i=0;i<chars.Length;i++)chars[i]=(char)(Convert.ToByte(hex.Substring(i*4,2),16)|(Convert.ToByte(hex.Substring(i*4+2,2),16)<<8));return new string(chars);}
 public static void Policy(Package p){Case.Features=p.features.ToDictionary(x=>x.key,x=>x.value??"");Case.CurrentVer=Case.Features["CurrentVer"];LoginContents.DormantReward=p.rewards.Select(x=>new DormantReward{RewardItemClass=Decode(x.item_utf16),Num=x.num,MinLevel=x.min_level,MaxLevel=x.max_level,Feature=x.feature}).ToList();}
 public static string Digest(IEnumerable<byte[]> calls){using(var s=new MemoryStream())using(var w=new BinaryWriter(s)){w.Write(System.Text.Encoding.ASCII.GetBytes("DormantRewardPlan\0v1"));foreach(var call in calls){w.Write(call.Length);w.Write(call);}using(var sha=SHA256.Create())return BitConverter.ToString(sha.ComputeHash(s.ToArray())).Replace("-","").ToLowerInvariant();}}
 public static void Main(string[] args){Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var serializer=new JavaScriptSerializer{MaxJsonLength=512*1024};var package=serializer.Deserialize<Package>(File.ReadAllText(args[0]));Policy(package);
  var versionSet=new[]{"0","1","1.09","1.1","3.01.01","3.09.00","3.1","3.10","10"};var pairs=new List<object>();
  foreach(var culture in new[]{"","en-US","zh-TW","zh-CN","tr-TR"}) {CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);foreach(var current in versionSet)foreach(var value in versionSet)pairs.Add(new{culture,current,value,enabled=current.CompareTo(value)>=0});}
  CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo("en-US");
  var expressionMap=new Dictionary<string,string>{{"CurrentVer","3.09.00"},{"a","1.1"},{"b",""},{"Case","1"},{"single||","1"},{"n","9"},{"okint"," +12 "},{"invalidint","x"},{"overflowint","2147483648"}};
  Case.Features=expressionMap;Case.CurrentVer="3.09.00";var expressions=new List<object>();
  foreach(var value in new string[]{null,"","a","b","missing","!missing","!","!!","a||b","b&&a","a&&missing||a"," a ","\u00A0a\u3000"," ","\t","a||","||a","||","a&&","Case","case","!!a","a||!a","!a&&b"}) {try{expressions.Add(new{expression=value,enabled=(bool?)ServiceCore.FeatureMatrix.IsEnable(value),error=(string)null});}catch(Exception e){expressions.Add(new{expression=value,enabled=(bool?)null,error=e.GetType().Name});}}
  var integers=new List<object>();int integerIndex=0;foreach(var input in new string[]{" +12 ","x","2147483648",null,"-2147483648","+2147483647","-2147483649","0000000000000000000000000012","-0","+","1.0","1,000","\u00A012\u00A0","\u300012\u3000","\t12\r\n","12\0","12\0\0","12 \0","12\0 ","\012","  -0012  "}){var key="integer"+(integerIndex++);Case.Features[key]=input??"";try{integers.Add(new{key,input,value=(int?)ServiceCore.FeatureMatrix.GetInteger(key),error=(string)null});}catch(Exception e){integers.Add(new{key,input,value=(int?)null,error=e.GetType().Name});}}
  Policy(package);var levels=new SortedSet<int>(new[]{int.MinValue,-1,0,1,20,60,100,int.MaxValue});foreach(var r in package.rewards){foreach(var b in new[]{r.min_level,r.max_level}){levels.Add(b);if(b>int.MinValue)levels.Add(b-1);if(b<int.MaxValue)levels.Add(b+1);}}
  var characters=new List<GetCharacters결과>();int index=0;foreach(var level in levels)characters.Add(new GetCharacters결과{ID=index++-100,Level=level,Name=index%3==0?null:index%3==1?"":Decode("4100000000D83DD842DE")});
  var context=RecoveredRewards.Run(characters);var plan=new{characters=characters.Select(x=>new{id=x.ID.ToString(CultureInfo.InvariantCulture),level=x.Level,name_utf16=x.Name==null?null:BitConverter.ToString(Unicode(x.Name)).Replace("-","")}).ToArray(),count=context.Calls.Count,digest=Digest(context.Calls)};
  var units=new SortedSet<long>();for(long i=-301;i<=601;i++)units.Add(i);foreach(var value in new[]{-1391644800000L,76683386879999L,25919999L,25920000L,25920001L})units.Add(value);
  var dates=new List<object>();string connection=Environment.GetEnvironmentVariable("REWARD_ORACLE_SQLSERVER");using(var conn=new SqlConnection(connection)){conn.Open();using(var cmd=conn.CreateCommand()){
   var selects=new List<string>();foreach(var u in units){int days=(int)Math.Floor(u/25920000.0);int time=(int)(u-(long)days*25920000);var sql=new SqlDateTime(days,time);var binary=BitConverter.GetBytes(days).Reverse().Concat(BitConverter.GetBytes(time).Reverse()).ToArray();selects.Add("SELECT CAST("+u+" AS bigint) AS units,CONVERT(datetime,0x"+BitConverter.ToString(binary).Replace("-","")+") AS value");dates.Add(new{units=u.ToString(CultureInfo.InvariantCulture),ticks=sql.Value.Ticks.ToString(CultureInfo.InvariantCulture)});}
   cmd.CommandText=string.Join(" UNION ALL ",selects);cmd.CommandTimeout=15;int row=0;using(var reader=cmd.ExecuteReader())while(reader.Read()){var u=reader.GetInt64(0);var value=reader.GetDateTime(1);long expected=Convert.ToInt64(dates[row].GetType().GetProperty("ticks").GetValue(dates[row],null));if(value.Ticks!=expected)throw new Exception("SqlClient date mismatch");row++;}if(row!=dates.Count)throw new Exception("SqlClient count mismatch");
  }}
  Console.WriteLine(serializer.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",version_pairs=pairs,expressions,integers,plan,dates,sqlclient_rows=dates.Count,game_services_executed=false}));
 }
}
