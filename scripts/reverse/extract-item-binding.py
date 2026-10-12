"""Extract reviewed binding predicates only; generated source remains ignored."""
from pathlib import Path

def method(source, marker):
    start=source.index(marker)
    opening=source.index("{",start)
    depth=1
    i=opening+1
    while depth:
        depth+=(source[i]=="{")-(source[i]=="}")
        i+=1
    return source[start:i]

root=Path("legacy/decompiled/PlayerService-f23d8b9df77c")
bind=method((root/"PlayerService.ItemServiceCore.TradeSystem/ItemGenerator.cs").read_text(encoding="utf-8-sig"),"public static bool CheckCharacterBind(bool")
high=method((root/"PlayerService.ItemServiceCore.ItemInstance/Item.cs").read_text(encoding="utf-8-sig"),"public static bool CheckHighLevelEquipment")
equip=method((root/"PlayerService.ItemServiceCore/ItemContentsExtension.cs").read_text(encoding="utf-8-sig"),"public static bool IsEquippable(this ItemClassInfo")
enum=(root/"PlayerService.ItemServiceCore.ItemInstance/GemstoneType.cs").read_text(encoding="utf-8-sig").split("public enum",1)[1]
text="using System; using System.Collections.Generic;\npublic static class ItemGenerator {"+bind+"}\npublic static class Item {"+high+"}\npublic static class ItemContentsExtension {"+equip+" public static object GetEquipItemInfo(this ItemClassInfo info) { return info.equippable ? info : null; }}\npublic enum"+enum
Path(".cache/framework/ItemBindingRecovered.cs").write_text(text,encoding="utf-8",newline="\n")
print("Reviewed binding predicates extracted; source services not executed")
