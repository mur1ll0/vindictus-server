using System;
using System.Collections.Generic;
using System.Web.Script.Serialization;
class LabClockOracle {
 static void Main() {
  var rows=new List<object>();
  long[] seconds={-62135596800L,-2208988800L,-1L,0L,946684800L,2147483648L,253402300799L};
  int[] nanos={0,99,100,3333333,999999999};
  foreach(long sec in seconds) foreach(int ns in nanos) foreach(int offset in new[]{-50400,0,28800,50400}) {
   long ticks; long? civil=null; bool utc=false;
   try { var time=new DateTime(1970,1,1,0,0,0,DateTimeKind.Utc).AddTicks(checked(sec*TimeSpan.TicksPerSecond+ns/100));ticks=time.Ticks;utc=true;
    try {civil=time.AddTicks((long)offset*10000000L).Ticks;}catch(ArgumentOutOfRangeException){}
   }catch(ArgumentOutOfRangeException){ticks=0;}
   rows.Add(new {seconds=sec.ToString(),nanoseconds=ns,offset_seconds=offset,utc_valid=utc,utc_ticks=utc?ticks.ToString():null,civil_ticks=civil.HasValue?civil.Value.ToString():null});
  }
  Console.Write(new JavaScriptSerializer().Serialize(new {architecture=IntPtr.Size==4?"x86":"x64",cases=rows}));
 }
}
