using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Web.Script.Serialization;
using ServiceCore.EndPointNetwork;
class BaseHarness { protected bool Finished; }
class Journal { public void AddJournal(string s) {} }
class DummyLogger { public void ErrorFormat(string s,params object[] args) {} public void InfoFormat(string s,params object[] args) {} }
class Log<T> { public static DummyLogger Logger=new DummyLogger(); }
class LoginService { public string BannedReason; }
class Input {
    public bool? member; public string ip; public int seconds; public bool limited,dormant,returning;
    public byte[] token; public string banned;
}
class Harness:BaseHarness {
    public IEnumerable<object> Run(Input input) {
        var service=new LoginService { BannedReason=input.banned }; var userEntity=new Journal();
        string nexonID="synthetic"; bool? banResult=input.member;
        /*MEMBER*/
        string lastLoginIP=""; int lockTime=0;
        var lockResult=new KeyValuePair<string,int>(input.ip,input.seconds);
        /*IP*/
        string facebookToken=input.token==null ? "" : Encoding.GetEncoding(1252).GetString(input.token);
        int limited=input.limited?1:0; bool isDormant=input.dormant, isReturn=input.returning;
        /*YIELDS*/
    }
}
class Oracle {
    static int[] Units(string s){return s==null?null:s.Select(c=>(int)c).ToArray();}
    static object Field(object o){
        if(o is LoginFailMessage.FailReason)return new {kind="reason",number=(int)o};
        if(o is int)return new {kind="int",number=(int)o};
        if(o is bool)return new {kind="bool",flag=(bool)o};
        return new {kind="text",units=Units((string)o)};
    }
    static void Main(){
        var cases=new List<object>();
        foreach(bool? member in new bool?[]{null,true,false})
        foreach(string ip in new[]{null,"","127.0.0.1","\0\ud800"})
        foreach(int seconds in new[]{int.MinValue,-1,0,600,int.MaxValue})
        foreach(int bits in Enumerable.Range(0,8)){
            var input=new Input{member=member,ip=ip,seconds=seconds,limited=(bits&1)!=0,dormant=(bits&2)!=0,returning=(bits&4)!=0,
                token=bits%3==0?null:bits%3==1?new byte[0]:new byte[]{0,0x80,0x81,0xff},banned=bits%2==0?null:"synthetic ban\0\ud800"};
            cases.Add(new {input=new {member=input.member,ip=Units(input.ip),seconds,limited=input.limited,dormant=input.dormant,returning=input.returning,
                token=input.token==null?null:input.token.Select(b=>(int)b).ToArray(),banned=Units(input.banned)},
                output=new Harness().Run(input).Select(Field).ToArray()});
        }
        var decoding=Enumerable.Range(0,256).Select(n=>new {value=n,units=Units(Encoding.GetEncoding(1252).GetString(new[]{(byte)n}))}).ToArray();
        Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=5000000}.Serialize(new {architecture=IntPtr.Size==8?"x64":"x86",cases,decoding}));
    }
}
namespace ServiceCore.EndPointNetwork { public interface IMessage {} }
