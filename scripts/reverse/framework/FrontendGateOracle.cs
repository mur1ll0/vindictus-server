using System;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using ServiceCore.LoginServiceOperations;
using ServiceCore.EndPointNetwork;
class CheckPassport { public LoginFailMessage.FailReason Reason; public bool IsOnline; public string WebShutDownMessage = ""; public string NexonID = "synthetic"; }
class Parent { public Permissions Permissions; }
class BaseStub { public Parent Parent = new Parent(); }
class LoggerStub { public void WarnFormat(string text, params object[] args) {} }
static class Log<T> { public static LoggerStub Logger = new LoggerStub(); }
static class PermissionShim { public static bool Contains(this Permissions value, Permissions required) { return (value & required) == required; } }
class PassportCheck : BaseStub {
    int result;
    void MoveToInitialState(object value) {
        result = (int)(value is CheckPassport ? ((CheckPassport)value).Reason : (LoginFailMessage.FailReason)value);
    }
    void Execute(int bits, bool online, int failureReason) {
        result = -1;
        base.Parent.Permissions = (Permissions)bits;
        CheckPassport checkPassport = new CheckPassport { IsOnline = online, Reason = (LoginFailMessage.FailReason)failureReason };
        /* RECOVERED_GATE */
    }
    public int Run(int bits, bool online, int failureReason) { Execute(bits, online, failureReason); return result;
    }
}
namespace ServiceCore.EndPointNetwork { public interface IMessage {} }
class Oracle {
    static void Main() {
        var cases = new List<object>();
        int[] unrelated = { 0, 0x2000000, 0x1000000, unchecked((int)0x80000000), 0x20000000, 0x800000 };
        foreach (int extra in unrelated) for (int low=0;low<4;low++) foreach (bool online in new[]{false,true}) foreach(int reason in new[]{1,4,13}) {
            int bits = extra | low;
            cases.Add(new {bits, online, reason, result = new PassportCheck().Run(bits,online,reason)});
        }
        var secured = new List<object>();
        for (int code=0;code<256;code++) {
            byte SecureCode = (byte)code;
            bool value = /* RECOVERED_OTP */;
            secured.Add(new { code, value });
        }
        Console.WriteLine(new JavaScriptSerializer().Serialize(new { architecture=IntPtr.Size==8?"x64":"x86", cases, secured }));
    }
}
