using System;
using System.Collections.Generic;
using System.Globalization;
using System.Threading;
using System.Web.Script.Serialization;
// Synthetic harness: no reference to or execution of original assemblies.
// Identity expressions adapted from PassportCheck.OnEnter (recovered source lines 146-214).
class IdentityOracle {
    static object Identity(string passport, bool zhCN) {
        if(passport == null) return new { error = "NullPassport", nexon_id = (string)null, nexon_sn = (int?)null };
        if(zhCN) {
            string suffix = null; int minimum = 1, maximum = 0, length = 0;
            if(passport.ToLower().StartsWith("cmhtest")) { suffix = passport.Substring(7); maximum = 60; length = 2; }
            else if(passport.ToLower().StartsWith("whb")) { suffix = passport.Substring(3); maximum = 6; length = 3; }
            int number;
            if(suffix == null || suffix.Length != length || !int.TryParse(suffix, out number) || number < minimum || number > maximum)
                return new { error = "NoUserInfo", nexon_id = (string)null, nexon_sn = (int?)null };
        }
        if(passport.Length == 0) return new { error = "PassportFail", nexon_id = (string)null, nexon_sn = (int?)null };
        return new { error = (string)null, nexon_id = passport.Length > 24 ? passport.Substring(0, 24) : passport, nexon_sn = (int?)passport.ToLower().GetHashCode() };
    }
    static int[] Units(string value) { if(value == null) return null; var units = new int[value.Length]; for(int i=0;i<value.Length;i++) units[i]=value[i]; return units; }
    static void Main() {
        var values = new List<string> { null, "", "a", "A", "ab", "abc", "abcd", "abcde", "abcdef", "ABC_DEF-169", "I", "FILE", "CMHTEST01", "cmhtest60", "cmhtest61", "cmhtest00", "cmhtest+1", "cmhtest 1", "cmhtest1 ", "WHB001", "whb006", "whb007", "whb +1", "whb1  ", "whb-01", "whb01", "whb000", new string('A', 24), new string('A', 25), new string('A', 24)+"b", new string('A', 24)+"c", "a\0b", "\u00c9\u03a3\u0130", "\ud83d\ude00" };
        var random = new Random(169);
        for(int length = 1; length <= 160; length++) { var chars = new char[length]; for(int i=0;i<length;i++) chars[i]=(char)random.Next(32,127); values.Add(new string(chars)); }
        var profiles = new List<object>();
        foreach(var culture in new [] { "", "en-US", "zh-TW", "zh-CN", "tr-TR" }) {
            Thread.CurrentThread.CurrentCulture = culture == "" ? CultureInfo.InvariantCulture : CultureInfo.GetCultureInfo(culture);
            var cases = new List<object>();
            foreach(var value in values) {
                bool supported = value == null || Array.TrueForAll(value.ToCharArray(), c => c >= 32 && c <= 126);
                cases.Add(new { passport = value, supported_ascii = supported, lower_utf16 = Units(value == null ? null : value.ToLower()), raw_utf16 = Units(value), raw_hash = value == null ? (int?)null : value.GetHashCode(), generic = Identity(value, false), zh_cn = Identity(value, true) });
            }
            profiles.Add(new { culture = culture, cases = cases });
        }
        Console.WriteLine(new JavaScriptSerializer().Serialize(new { runtime = Environment.Version.ToString(), framework_file_version = System.Diagnostics.FileVersionInfo.GetVersionInfo(typeof(string).Assembly.Location).FileVersion, architecture = IntPtr.Size == 4 ? "x86" : "x64", profiles = profiles }));
    }
}

