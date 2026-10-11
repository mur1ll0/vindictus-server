using System;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using ServiceCore.EndPointNetwork;
class Log<T> { public static Logger Logger = new Logger(); }
class Logger { public void WarnFormat(string text, params object[] args) {} public void ErrorFormat(string text, params object[] args) {} }
class Operation { public string ID = "synthetic"; }
class Connection { public string RemoteCategory; public Entity LocalEntity; }
class Entity {
    public long ID; public object Tag; public int UseCount; public int CloseCount;
    public event EventHandler Used; public event EventHandler Closed;
    public void RaiseUsed() { if(Used!=null) Used(this,EventArgs.Empty); }
    public void Close() { CloseCount++; if(Closed!=null) Closed(this,EventArgs.Empty); }
}
class UserEntity { public DateTime FID=new DateTime(2000,1,1); public string NexonID="synthetic"; public int Journals; public void AddJournal(string text) { Journals++; } }
class DisconnectClient {}
class Location { public Location(long id,string category) {} }
static class Job { public static Action Create(Action action) { return action; } }
class ThreadStub { public List<Action> Jobs=new List<Action>(); public void Enqueue(Action action) { Jobs.Add(action); } }
class ServiceStub { public int Disconnects; public ThreadStub Thread=new ThreadStub(); public void RequestOperation(Entity entity,Location target,DisconnectClient operation) { Disconnects++; } }
class ProcessorBase { public Connection Connection; public Operation Operation=new Operation(); public bool Finished; }
class UserLoginProcessor : ProcessorBase {
    public ServiceStub service = new ServiceStub();
    public IEnumerable<object> Run() {
        /* RECOVERED_DUPLICATE */
        yield return "continue";
    }
}
class EntityDataContext {
    public static bool Throw; public static int Calls; public static long EntityID; public static string Category; public static int Before; public static int After;
    public int AcquireService(long id,string category,int after,int before) {
        Calls++;EntityID=id;Category=category;Before=before;After=after;
        if(Throw)throw new Exception("synthetic");
        return 0;
    }
}
class ServiceBase {
    public int ID=42; public string Category="LoginServiceCore.LoginService";
    public virtual Entity MakeEntity(long id,string category) { return new Entity {ID=id}; }
}
class LoginService : ServiceBase {
    /* RECOVERED_ENTITY */
}
namespace ServiceCore.EndPointNetwork { public interface IMessage {} }
class Oracle {
    static void Main() {
        var duplicates=new List<object>();
        foreach(string remote in new[]{"FrontendServiceCore.FrontendService","Other","frontendservicecore.frontendservice",null})
        foreach(string tag in new[]{"null","user","other"}) {
            object value=tag=="user"?(object)new UserEntity():tag=="other"?new object():null;
            var entity=new Entity { ID=-1,Tag=value };
            var processor=new UserLoginProcessor {Connection=new Connection {RemoteCategory=remote,LocalEntity=entity}};
            var feedback=new List<string>();
            foreach(object item in processor.Run()) feedback.Add(item is LoginFailMessage.FailReason?((int)item).ToString():item.ToString());
            int before=entity.CloseCount;
            foreach(Action job in processor.service.Thread.Jobs) job();
            duplicates.Add(new {remote,tag,feedback,finished=processor.Finished,disconnects=processor.service.Disconnects,queued=processor.service.Thread.Jobs.Count,close_before=before,close_after=entity.CloseCount});
        }
        var lifecycle=new List<object>();
        foreach(long id in new[]{(long)int.MinValue,0L,(long)int.MaxValue})
        foreach(int uses in new[]{0,1,3})
        foreach(bool throws in new[]{false,true}) {
            EntityDataContext.Throw=throws;EntityDataContext.Calls=0;
            var entity=new LoginService().MakeEntity(id,"LoginServiceCore.LoginService");
            entity.Tag=new UserEntity();entity.UseCount=uses;
            entity.RaiseUsed();
            if(uses!=0)entity.Close();
            lifecycle.Add(new {id,uses,throws,closed=entity.CloseCount,tag_cleared=entity.Tag==null,calls=EntityDataContext.Calls,before=EntityDataContext.Before,after=EntityDataContext.After});
        }
        Console.WriteLine(new JavaScriptSerializer().Serialize(new{architecture=IntPtr.Size==8?"x64":"x86",duplicates,lifecycle}));
    }
}

