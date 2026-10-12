using System;
using System.Collections.Generic;
using System.Data;
using System.Data.SqlClient;
using System.Data.SqlTypes;
using System.Web.Script.Serialization;
public static class Program {
 public static void Main() {
  Console.OutputEncoding=new System.Text.UTF8Encoding(false);
  var ticks=new SortedSet<long>();
  long epoch=new DateTime(1900,1,1).Ticks, day=TimeSpan.TicksPerDay;
  foreach(long origin in new long[]{epoch-day,epoch,epoch+day,new DateTime(1753,1,1).Ticks,new DateTime(9999,12,31).Ticks}) {
   foreach(long delta in new long[]{0,1,16665,16666,16667,16668,49999,50000,50001,83332,83333,83334,99999,100000,100001,day-16668,day-16667,day-16666,day-1}) ticks.Add(origin+delta);
  }
  foreach(long t in new long[]{0,new DateTime(1753,1,1).Ticks-1,DateTime.MaxValue.Ticks,DateTime.MaxValue.Ticks-1})ticks.Add(t);
  var cases=new List<object>();
  using(var connection=new SqlConnection(Environment.GetEnvironmentVariable("DATETIME_ORACLE_SQLSERVER"))) {
   connection.Open();
   foreach(long t in ticks) {
    var date=new DateTime(t,DateTimeKind.Utc);string units=null,error=null,sql=null;
    try {var d=new SqlDateTime(date);units=((long)d.DayTicks*25920000+d.TimeTicks).ToString(System.Globalization.CultureInfo.InvariantCulture);}catch(Exception e){error=e.GetType().Name;}
    try {using(var command=new SqlCommand("SELECT CONVERT(binary(8),@p)",connection)){
     command.Parameters.Add("@p",SqlDbType.DateTime).Value=date;
     byte[] bytes=(byte[])command.ExecuteScalar();uint time=((uint)bytes[4]<<24)|((uint)bytes[5]<<16)|((uint)bytes[6]<<8)|bytes[7];int days=(bytes[0]<<24)|(bytes[1]<<16)|(bytes[2]<<8)|bytes[3];sql=((long)days*25920000+time).ToString(System.Globalization.CultureInfo.InvariantCulture);
    }}catch(Exception){sql=null;}
    if(units!=sql)throw new Exception("SqlDateTime and SqlClient disagree on synthetic input");
    cases.Add(new {ticks=t.ToString(System.Globalization.CultureInfo.InvariantCulture),units300=units,error=error});
   }
  }
  Console.WriteLine(new JavaScriptSerializer().Serialize(new {architecture=IntPtr.Size==4?"x86":"x64",cases=cases}));
 }
}
