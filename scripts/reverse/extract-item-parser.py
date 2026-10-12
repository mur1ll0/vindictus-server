"""Extract syntax/build/equality unchanged; inject only the argument resolver."""
from pathlib import Path
source=Path("legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemClassExBuilder.cs").read_text(encoding="utf-8")
start=source.index("\n{",source.index("public class ItemClassExBuilder"))
end=source.index("\n\tprivate static int ToArgInt",start)
suffix=source.index("\n\tpublic static bool IsSameItemClassEx",end)
body=source[start+2:end]
body=body.replace("ToArgInt(", "Case.Resolve(")
# Keep the original syntax, build and equality bodies in ignored generated output.
body+=source[suffix:source.rfind("}")]
wrapper="""using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using Utility;
namespace ServiceCore.ItemServiceOperations {
public class ItemClassExBuilder {
BODY
}
}
"""
Path(".cache/framework/ItemParserRecovered.cs").write_text(wrapper.replace("BODY",body),encoding="utf-8",newline="\n")
