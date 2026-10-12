using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;
using System.Web.Script.Serialization;

public static class Log<T> { public static readonly Sink Logger = new Sink(); }
public sealed class Sink { public void WarnFormat(string format, params object[] args) {} }
public static class Case {
    public static readonly List<string> Calls = new List<string>();
    public static object Parsed;
    public static bool? Suffix;
    public static DateTime Clock;
    public static TimeZoneInfo Zone;
    public static bool ParseDate(string text, out DateTime result) {
        Calls.Add("date");
        bool success=DateTime.TryParse(text, out result);
        Parsed=success ? Pack(result) : null;
        return success;
    }
    public static bool EndsWithSeconds(string text) {
        Calls.Add("suffix");
        Suffix=text.EndsWith("sec");
        return Suffix.Value;
    }
    public static DateTime Now { get { Calls.Add("clock"); return Clock; } }
    public static DateTime LocalToUtc(DateTime local) {
        Calls.Add("utc");
        return TimeZoneInfo.ConvertTimeToUtc(DateTime.SpecifyKind(local,DateTimeKind.Unspecified),Zone);
    }
    public static object Pack(DateTime date) {
        return new { ticks=date.Ticks.ToString(CultureInfo.InvariantCulture),kind=date.Kind.ToString() };
    }
    public static object Run(string input, DateTime clock) {
        Calls.Clear(); Parsed=null; Suffix=null; Clock=clock;
        int? value=null; string error=null;
        try { value=ItemClassExBuilder.ToArgInt(input); }
        catch(ArgumentOutOfRangeException) { error="DateRange"; }
        var units=input==null ? null : Hex(input);
        return new {input_hex=units,clock=Pack(clock),parsed=Parsed,suffix=Suffix,
            calls=Calls.ToArray(),value=value,error=error, offset_minutes=(int)Zone.BaseUtcOffset.TotalMinutes};
    }
    private static string Hex(string text) {
        var bytes=new byte[text.Length*2];
        for(int i=0;i<text.Length;i++){bytes[2*i]=(byte)text[i];bytes[2*i+1]=(byte)(text[i]>>8);}
        return BitConverter.ToString(bytes).Replace("-","").ToLowerInvariant();
    }
}
public static class Program {
    public static void Main() {
        Console.OutputEncoding=new UTF8Encoding(false);
        CultureInfo.CurrentCulture=new CultureInfo("en-US",false);
        Case.Zone=TimeZoneInfo.CreateCustomTimeZone("reference-fixed-zone",TimeSpan.FromMinutes(480),"reference-fixed-zone","reference-fixed-zone");
        var inputs=new List<string>{null,""," ","01/02/2020","02/01/2020","0","-2147483648","2147483648",
            "0sec","1sec","-1sec","+3600sec","2147483647sec","-2147483648sec",
            "2147483648sec","sec"," 1sec","1sec ","1 sec","\t1sec","\n1sec\r",
            "0000-01-01","2020-00-01","2020-13-01","2020-01-00","2020-01-32"};
        foreach(int year in new[]{1,4,100,400,1600,1900,1969,2000,2038,9999})
        foreach(int month in new[]{1,2,4,12})
        foreach(int day in new[]{1,28,29,30,31}) {
            inputs.Add(string.Format(CultureInfo.InvariantCulture,"{0:D4}-{1:D2}-{2:D2}",year,month,day));
            inputs.Add(string.Format(CultureInfo.InvariantCulture,"{0}/{1}/{2:D4}",month,day,year));
        }
        foreach(string date in new[]{"0001-01-01","1969-12-31","2038-01-19","9999-12-31"})
        foreach(string time in new[]{"00:00:00","01:02:03","12:00:00","23:59:59",
            "00:00:00.1","01:02:03.12","12:34:56.123","23:59:59.1234",
            "00:00:00.12345","01:02:03.123456","23:59:59.9999999",
            "24:00:00","23:60:00","23:59:60"}) {
            inputs.Add(date+" "+time);
            inputs.Add(date+"T"+time);
        }
        foreach(string date in new[]{"1/1/0001","12/31/1969","1/19/2038","12/31/9999"})
        foreach(string time in new[]{"1:02:03 AM","12:00:00 AM","1:02:03 PM","12:00:00 PM","1:02:03 am","1:02:03 pm","11:59:59.9999999 PM"})
            inputs.Add(date+" "+time);
        foreach(string text in new[]{"2020-02-29","02/29/2020","2020-01-02 03:04:05"})
            inputs.Add("\t "+text+" \r\n");
        inputs.AddRange(new[]{"1/2/2020T03:04:05","01/02/2020T03:04:05",
            "12/31/9999T23:59:59.9999999","2020-01-02 1:02:03",
            "1/2/2020 03:04:05.1234567","2020-01-02 01:02:03 aM"});
        var unsupported=new List<string>{"Jan 2, 2020","01/02/20","2020-1-2","3:04:05","2020-01",
            "2020-01-02T03:04:05Z","2020-01-02T03:04:05+08:00","2020-01-02 03:04",
            "2020-01-02 03:04:05.12345678","2020-01-02  03:04:05","1SEC","1\u00a0sec","1se\0c","1sec\0",
            "2020-01-02 0:00:00 AM","2020-01-02T01:00:00 PM"};
        var profiles=new List<object>();
        foreach(long clockTicks in new[]{0L,621355968000000001L,638397504001234567L,3155378975999999999L}) {
            var utc=new DateTime(clockTicks,DateTimeKind.Utc);
            var local=DateTime.SpecifyKind(TimeZoneInfo.ConvertTimeFromUtc(utc,Case.Zone),DateTimeKind.Local);
            var rows=new List<object>();
            foreach(string text in inputs)rows.Add(Case.Run(text,local));
            profiles.Add(new{utc_clock=Case.Pack(utc),cases=rows});
        }
        var unsupportedHex=new List<string>();
        foreach(string text in unsupported){
            var bytes=new byte[text.Length*2];
            for(int i=0;i<text.Length;i++){bytes[2*i]=(byte)text[i];bytes[2*i+1]=(byte)(text[i]>>8);}
            unsupportedHex.Add(BitConverter.ToString(bytes).Replace("-","").ToLowerInvariant());
        }
        Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=33554432}.Serialize(new{
            architecture=IntPtr.Size==4?"x86":"x64",culture="en-US",offset_minutes=480,profiles=profiles,unsupported_hex=unsupportedHex}));
    }
}
