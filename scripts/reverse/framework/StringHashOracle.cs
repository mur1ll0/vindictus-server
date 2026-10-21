using System;
using System.Collections.Generic;
using System.Globalization;
using System.Web.Script.Serialization;
// String.GetHashCode of the .NET Framework 4.8 runtime (no randomized hashing) and the
// first System.Random(hash).Next(n) values, as TodayQuest seeds them with Year+DayOfYear.
public static class StringHashOracle {
 public static void Main() {
  CultureInfo.CurrentCulture = new CultureInfo("en-US");
  var texts = new List<string> { "", "a", "ab", "abc", "abcd", "abcde", "\u00e9", "\uac00\ub098\ub2e4", "quest_gnoll_watchman", "S3CH1" };
  for (int year = 2008; year <= 2040; year += 4) {
   foreach (int day in new[] { 1, 9, 10, 99, 100, 200, 294, 365, 366 }) texts.Add(year.ToString() + day);
  }
  var date = new DateTime(2026, 1, 1);
  for (int i = 0; i < 400; i += 7) { var d = date.AddDays(i); texts.Add(d.Year.ToString() + d.DayOfYear); }
  var cases = new List<object>();
  foreach (string text in texts) {
   int hash = text.GetHashCode();
   var random = new Random(hash);
   var next = new List<int>();
   foreach (int n in new[] { 1, 2, 3, 7, 40, 190 }) next.Add(random.Next(n));
   cases.Add(new Dictionary<string, object> { { "text", text }, { "hash", hash }, { "next", next } });
  }
  Console.WriteLine(new JavaScriptSerializer { MaxJsonLength = 1 << 24 }.Serialize(new { architecture = IntPtr.Size == 4 ? "x86" : "x64", cases = cases }));
 }
}
