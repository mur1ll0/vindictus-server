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
    public static DateTime? Injected;
    public static bool ParseDate(string text, out DateTime result) {
        Calls.Add("date");
        bool success;
        if(Injected.HasValue) { result=Injected.Value; success=true; }
        else success=DateTime.TryParse(text, out result);
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
    public static object Run(string input, DateTime clock, DateTime? injected) {
        Calls.Clear(); Parsed=null; Suffix=null; Clock=clock; Injected=injected;
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
        var groups=new List<object>();
        var inputs=new string[] {null,"","0","-1","+2147483647","-2147483648","2147483648","42\0",
            "invalid","2020-01-02","01/02/2020","31/12/2020","12/31/2020","2020-02-30",
            "1970-01-01 00:00:00","1969-12-31 23:59:59.9999999","2038-01-19 03:14:08",
            "9999-12-31 23:59:59","0001-01-01","1sec","-1sec","+3600sec","0sec",
            "2147483647sec","-2147483648sec","2147483648sec","sec","1SEC","1sec ",
            "1se\0c","1sec\0"," 1sec","\u00a01sec","1\0sec"};
        var clocks=new long[] {0,1,621355967999999999,621355968000000001,638397504001234567,3155378975999999999};
        var dates=new long[] {0,1,621355967989999999,621355967990000001,621355967999999999,
            621355968000000000,621355968000000001,621355968010000001,
            642830804470000000,642830804480000000,664305640960000000,3155378975999999999};
        foreach(string culture in new[]{"en-US","pt-BR","zh-TW","tr-TR","fr-FR"}) {
            CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);
            foreach(int offset in new[]{0,480,-180}) {
                Case.Zone=TimeZoneInfo.CreateCustomTimeZone("oracle-zone",TimeSpan.FromMinutes(offset),"oracle-zone","oracle-zone");
                var rows=new List<object>();
                // Date parsing without a timezone stays Unspecified and is host independent.
                foreach(string text in inputs) rows.Add(Case.Run(text,new DateTime(clocks[4],DateTimeKind.Local),null));
                foreach(long clock in clocks) foreach(string text in new[]{"0sec","1sec","-1sec","2147483647sec","-2147483648sec"})
                    rows.Add(Case.Run(text,new DateTime(clock,DateTimeKind.Local),null));
                foreach(long ticks in dates) foreach(DateTimeKind kind in new[]{DateTimeKind.Unspecified,DateTimeKind.Utc,DateTimeKind.Local})
                    rows.Add(Case.Run("injected-date",new DateTime(clocks[4],DateTimeKind.Local),new DateTime(ticks,kind)));
                groups.Add(new {culture=culture,offset_minutes=offset,cases=rows});
            }
        }
        Console.WriteLine(new JavaScriptSerializer {MaxJsonLength=16777216}.Serialize(new {
            architecture=IntPtr.Size==4?"x86":"x64",profiles=groups}));
    }
}
