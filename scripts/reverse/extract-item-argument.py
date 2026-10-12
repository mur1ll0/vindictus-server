"""Extract only reviewed source methods; generated legacy code stays ignored."""
from pathlib import Path

def method(source, marker):
    start=source.index(marker)
    opening=source.index("{",start)
    depth=1
    i=opening+1
    while depth:
        depth += (source[i]=="{")-(source[i]=="}")
        i+=1
    return source[start:i]

source=Path("legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemClassExBuilder.cs").read_text(encoding="utf-8-sig")
utility=Path("legacy/decompiled/Utility-37359981d509/Utility/Extention.cs").read_text(encoding="utf-8-sig")
decision=method(source,"private static int ToArgInt").replace("private static","public static")
decision=decision.replace("DateTime.TryParse(str, out var result2)","Case.ParseDate(str, out var result2)")
decision=decision.replace('str.EndsWith("sec")',"Case.EndsWithSeconds(str)")
decision=decision.replace("DateTime.Now","Case.Now")
conversion=method(utility,"public static int ToInt32").replace("value.ToUniversalTime()","Case.LocalToUtc(value)")
generated="using System; using Utility;\npublic class ItemClassExBuilder {"+decision+"}\nnamespace Utility { public static class Extention { private static readonly DateTime epoch = new DateTime(1970,1,1,0,0,0,DateTimeKind.Utc);"+conversion+"} }"
Path(".cache/framework/ItemArgumentRecovered.cs").write_text(generated,encoding="utf-8",newline="\n")
