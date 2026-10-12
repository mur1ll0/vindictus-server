"""Extract bounded source predicates; proprietary recovered output stays ignored."""
from pathlib import Path
import re

base = Path("legacy/decompiled/PlayerService-f23d8b9df77c")
controller = (base/"PlayerService.ItemServiceCore.Static/QueuedItemController.cs").read_text(encoding="utf-8")
loader = (base/"PlayerService.ItemServiceCore/DBLoader.cs").read_text(encoding="utf-8")
def section(text, start, end):
    begin = text.index(start)
    return text[begin:text.index(end, begin)]
outer = section(controller, "internal static int ProcessQueuedItem(", "private static bool DefaultCharacterBinderChecker")
binder = section(controller, "private static bool DefaultCharacterBinderChecker", "private static void ProcessQueuedItemRow")
prefix = section(controller, "MailType mailType = MailType.GMMail;", "using (new ItemTransactionScope")
expiry = re.search(r"if \((row.ExpireTime.HasValue && row.ExpireTime <= DateTime.UtcNow)\)", controller).group(1)
mail = re.findall(r"if \((row.MailTitle != null && row.MailContent != null)\)", controller)
assert len(mail) == 2
selection = section(loader, "public override IEnumerable<QueuedItem> GetQueuedItems()", "public override void RemoveQueuedItem")
selection = selection.replace("public override", "public").replace("IQueryable<QueuedItem>", "IEnumerable<QueuedItem>").replace("base.Context", "Context")
wrapper = """
using System;
using System.Collections.Generic;
using System.Linq;
public static class Recovered {
OUTER
BINDER
private static void ProcessQueuedItemRow(long journalID, ItemOwner target, QueuedItem row) {
 Case.Attempted++; if(Case.ThrowRow) throw new InvalidOperationException();
}
public static object Route(QueuedItem row) {
 PREFIX
 return new { mail_type=mailType.ToString(), binder=func == (Func<bool,ItemClassInfo,IDictionary<string,ItemAttributeElement>,bool>)DefaultCharacterBinderChecker ? "Default":"MobileApp",
 route=(EXPIRY) ? "Expired" : (!row.ItemID.HasValue ? ((MAIL) ? "CreateToMail":"CreateToInventory") : ((MAIL) ? "ExistingToMail":"ExistingToInventory")) };
}
public static bool Bind(bool bound, string itemClass) {
 return DefaultCharacterBinderChecker(bound,new ItemClassInfo {ItemClass=itemClass,Bind="synthetic"},null);
}
}
public class Loader {
 public Context Context=new Context(); public long OwnerID=7;
 SELECTION
}
"""
for key,value in [("OUTER",outer),("BINDER",binder),("PREFIX",prefix),("EXPIRY",expiry.replace("DateTime.UtcNow","Case.Now")),("MAIL",mail[0]),("SELECTION",selection)]:
    wrapper=wrapper.replace(key,value)
Path(".cache/framework/QueueDecisionsRecovered.cs").write_text(wrapper,encoding="utf-8",newline="\n")
