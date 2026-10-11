using System;
using System.Linq;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using ServiceCore.LoginServiceOperations;
using ServiceCore.EndPointNetwork;
namespace UnifiedNetwork.Cooperation {
    public class Operation { public virtual OperationProcessor RequestProcessor(){return null;} }
    public abstract class OperationProcessor { public object Feedback; public bool Result; public abstract IEnumerable<object> Run(); }
    public abstract class OperationProcessor<T>:OperationProcessor { protected T Operation; public OperationProcessor(T value){Operation=value;} }
}
namespace ServiceCore.EndPointNetwork { public interface IMessage {} }
class Oracle {
    static int[] Units(string value){return value==null?null:value.Select(c=>(int)c).ToArray();}
    static object Field(object value){
        if(value is LoginFailMessage.FailReason)return new{kind="reason",number=(int)value};
        if(value is int)return new{kind="int",number=(int)value};
        if(value is bool)return new{kind="bool",flag=(bool)value};
        return new{kind="text",units=Units((string)value)};
    }
    static object Run(string label,object[] feed){
        var op=new UserLogin();
        var proc=op.RequestProcessor();
        var iterator=proc.Run().GetEnumerator();
        int consumed=0;bool incomplete=false;string error=null;
        try {
            while(iterator.MoveNext()){
                if(consumed==feed.Length){incomplete=true;break;}
                proc.Feedback=feed[consumed++];
            }
        }catch(Exception e){error=e.GetType().Name;}
        finally{iterator.Dispose();}
        return new{label,input=feed.Select(Field).ToArray(),consumed,incomplete,error,result=proc.Result,
            package=op.PackageCount,ip=Units(op.LastLoginIP),seconds=op.LockLastSecs,limited=op.Limited,
            dormant=op.IsDormant,token=Units(op.FacebookToken),returning=op.IsReturn,
            reason=(int)op.Reason,banned=Units(op.BanedReason)};
    }
    static void Main(){
        var cases=new List<object>();
        cases.Add(Run("normal",new object[]{1,"127.0.0.1",600,1,true,"synthetic",false}));
        cases.Add(Run("nulls",new object[]{int.MinValue,null,int.MaxValue,int.MinValue,false,null,true}));
        cases.Add(Run("enum-number-is-package",new object[]{4,"",-1,0,false,"",false}));
        cases.Add(Run("utf16",new object[]{1,"\0\ud800",0,1,false,"\udfff",true}));
        for(int reason=0;reason<=24;reason++){
            cases.Add(Run("reason-"+reason,reason==4?new object[]{(LoginFailMessage.FailReason)reason,"synthetic ban"}:new object[]{(LoginFailMessage.FailReason)reason}));
        }
        foreach(int reason in new[]{-1,999,int.MaxValue})cases.Add(Run("unknown-"+reason,new object[]{(LoginFailMessage.FailReason)reason}));
        cases.Add(Run("banned-null",new object[]{LoginFailMessage.FailReason.Banned,null}));
        cases.Add(Run("banned-empty",new object[]{LoginFailMessage.FailReason.Banned,""}));
        cases.Add(Run("wrong-ip",new object[]{1,true,0,0,false,"",false}));
        cases.Add(Run("wrong-seconds",new object[]{1,"",false,0,false,"",false}));
        cases.Add(Run("wrong-limited",new object[]{1,"",0,null,false,"",false}));
        cases.Add(Run("wrong-dormant",new object[]{1,"",0,0,1,"",false}));
        cases.Add(Run("wrong-token",new object[]{1,"",0,0,false,1,false}));
        cases.Add(Run("wrong-return",new object[]{1,"",0,0,false,"",null}));
        cases.Add(Run("wrong-banned",new object[]{LoginFailMessage.FailReason.Banned,1}));
        cases.Add(Run("truncated",new object[]{1,"",0}));
        cases.Add(Run("extra",new object[]{1,"",0,0,false,"",false,123}));
        cases.Add(Run("false-ambiguous",new object[]{false}));
        var single=new List<object>();
        for(int count=0;count<4;count++){
            try {var row=Enumerable.Range(0,count).Select(_=>new object()).SingleOrDefault();
                single.Add(new{count,limited=row!=null,error=(string)null});}
            catch(Exception e){single.Add(new{count,limited=false,error=e.GetType().Name});}
        }
        Console.WriteLine(new JavaScriptSerializer().Serialize(new{architecture=IntPtr.Size==8?"x64":"x86",cases,single}));
    }
}
