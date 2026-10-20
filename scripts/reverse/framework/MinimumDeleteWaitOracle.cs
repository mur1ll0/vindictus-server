using System;
using System.Collections.Generic;
using System.Web.Script.Serialization;
class MinimumDeleteWaitOracle {
    static int Calculate(int delay, DateTime? deleted, Func<DateTime> utc) {
        // SOURCE_BLOCK
        return num4;
    }
    static void Main() {
        var cases=new List<object>();
        long anchor=638448480000000000L;
        foreach(int delay in new[]{0,1,-1,1440,int.MinValue,int.MaxValue})
        foreach(long delta in new[]{0L,1L,-1L,9999999L,10000000L,-10000000L,21474836480000000L,-21474836490000000L})
        foreach(bool absent in new[]{false,true}) {
            int reads=0;
            Func<DateTime> utc=()=>new DateTime(++reads==1?anchor:anchor-delta,DateTimeKind.Utc);
            int result=Calculate(delay,absent?(DateTime?)null:new DateTime(anchor-delta,DateTimeKind.Utc),utc);
            cases.Add(new{delay,delta=delta.ToString(),absent,reads,result});
        }
        Console.Write(new JavaScriptSerializer().Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases}));
    }
}
