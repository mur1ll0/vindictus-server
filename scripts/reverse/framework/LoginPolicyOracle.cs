using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Web.Script.Serialization;
using LoginServiceCore;

static class Clock {
    public static DateTime SetNow, CheckNow;
}
enum Permissions { Cheat }
static class FeatureMatrix {
    public static int Minutes;
    public static bool Notify;
    public static int GetInteger(string key) { return Minutes; }
    public static bool IsEnable(string key) { return Notify; }
}
static class ContentsDataContext {
    public static bool Cheat;
    public static HashSet<Permissions> GetPermissions(string name, IPAddress ip, long cid) {
        return Cheat ? new HashSet<Permissions> { Permissions.Cheat } : new HashSet<Permissions>();
    }
}
class LoggerStub {
    public static bool ErrorSeen;
    public void Error(object message, params object[] detail) { ErrorSeen = true; }
}
static class Log<T> { public static LoggerStub Logger = new LoggerStub(); }
class UserDBDataContext : IDisposable {
    public static GetUserBanInfo결과 Selected;
    public static bool Fail;
    public IEnumerable<GetUserBanInfo결과> GetUserBanInfo(int? id) {
        if(Fail) throw new InvalidOperationException("synthetic database failure");
        return Selected == null ? new GetUserBanInfo결과[0] : new[] { Selected };
    }
    public void Dispose() {}
}
class LoginService {
    internal readonly int BannedReason_AccountStolen = 1;
    public string BannedReason { get; set; }
    internal Func<int,string,bool?> GetMemberState { get; private set; }
    public LoginService() {
        /* MEMBER_STATE */
    }
    /* CHECK_IP */
}
class LoginPolicyOracle {
    static string T(DateTime? dt) { return dt.HasValue ? dt.Value.Ticks.ToString() : null; }
    static string Hex(string text) {
        if(text == null) return null;
        var bytes = new List<byte>();
        foreach(char ch in text) { bytes.Add((byte)ch); bytes.Add((byte)(ch >> 8)); }
        return String.Concat(bytes.Select(b => b.ToString("X2")));
    }
    static void AddIp(List<object> cases, string old, DateTime? until, int minutes, bool secured, bool cheat, string current, long advance) {
        var user = new User { Name = "PolicyFixture", LastLoginIP = old, IPLockTime = until };
        FeatureMatrix.Minutes = minutes; ContentsDataContext.Cheat = cheat;
        Clock.CheckNow = Clock.SetNow.AddTicks(advance); LoggerStub.ErrorSeen = false;
        var result = new LoginService().CheckLastLoginIP(user, IPAddress.Parse(current), secured);
        cases.Add(new {
            last_ip_hex = Hex(old), lock_ticks = T(until), minutes, secured, cheat, current,
            set_now_ticks = T(Clock.SetNow), check_now_ticks = T(Clock.CheckNow),
            returned_ip_hex = Hex(result.Key), seconds = result.Value,
            output_ip_hex = Hex(user.LastLoginIP), output_lock_ticks = T(user.IPLockTime),
            caught_error = LoggerStub.ErrorSeen,
        });
    }
    static void Main() {
        Clock.SetNow = new DateTime(2024,2,29,12,0,0,DateTimeKind.Local);
        var ipCases = new List<object>();
        var locks = new DateTime?[] { null, Clock.SetNow.AddTicks(-1), Clock.SetNow,
            Clock.SetNow.AddTicks(9999999), Clock.SetNow.AddSeconds(1), Clock.SetNow.AddMinutes(10), DateTime.MaxValue };
        foreach(string old in new[] { null, "", "192.0.2.1", "192.0.2.2", "::1", "\uD800" })
        foreach(var until in locks)
        foreach(int minutes in new[] {-1,0,1,10})
        foreach(bool secured in new[] {false,true})
        foreach(bool cheat in new[] {false,true})
            AddIp(ipCases,old,until,minutes,secured,cheat,"192.0.2.1",0);
        AddIp(ipCases,"192.0.2.2",null,1,false,false,"192.0.2.1",5000000);
        AddIp(ipCases,"::1",null,10,false,false,"2001:0DB8:0:0:0:0:0:1",0);
        AddIp(ipCases,"::1",null,10,false,false,"::ffff:192.0.2.1",0);
        AddIp(ipCases,"192.0.2.1",Clock.SetNow.AddSeconds((double)int.MaxValue),10,false,false,"192.0.2.1",0);
        AddIp(ipCases,"192.0.2.1",Clock.SetNow.AddSeconds((double)int.MaxValue+1),10,false,false,"192.0.2.1",0);
        foreach(string current in new[] {"::1","2001:0DB8:0:0:0:0:0:1","::ffff:192.0.2.1","::192.0.2.1","::2","::FFFF","::1:0","::ffff:ffff"}) {
            AddIp(ipCases,null,null,10,false,false,current,0);
            AddIp(ipCases,"192.0.2.2",null,10,true,false,current,0);
        }
        Clock.SetNow = DateTime.MaxValue;
        AddIp(ipCases,"192.0.2.2",null,10,false,false,"192.0.2.1",0);
        var bans = new List<object>();
        foreach(int status in new[] {int.MinValue,-1,0,1,2,int.MaxValue})
        foreach(string reason in new[] { null,"","synthetic reason","\uD800" })
        foreach(bool notify in new[] {false,true}) {
            FeatureMatrix.Notify=notify; UserDBDataContext.Fail=false;
            UserDBDataContext.Selected=new GetUserBanInfo결과 { Status=status, Reason=reason };
            var service=new LoginService(); var decision=service.GetMemberState(42,"PolicyFixture");
            bans.Add(new { status,reason_hex=Hex(reason),notify,member_state=decision,output_reason_hex=Hex(service.BannedReason),query_failed=false });
        }
        UserDBDataContext.Selected=null;
        var missingService=new LoginService();
        bans.Add(new { status=(int?)null,reason_hex=(string)null,notify=false,member_state=missingService.GetMemberState(42,"PolicyFixture"),output_reason_hex=Hex(missingService.BannedReason),query_failed=false });
        UserDBDataContext.Fail=true;
        var failedService=new LoginService();
        bans.Add(new { status=(int?)null,reason_hex=(string)null,notify=false,member_state=failedService.GetMemberState(42,"PolicyFixture"),output_reason_hex=Hex(failedService.BannedReason),query_failed=true });
        Console.WriteLine(new JavaScriptSerializer { MaxJsonLength=4194304 }.Serialize(new {
            architecture=IntPtr.Size==4?"x86":"x64",runtime=Environment.Version.ToString(),
            ip_cases=ipCases, ban_cases=bans,
            method="Recovered method bodies; two DateTime.Now samples replaced by explicit clocks, external permissions/features/database/logger replaced by synthetic stubs",
        }));
    }
}
