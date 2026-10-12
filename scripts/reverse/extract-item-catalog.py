"""Extract reviewed catalog initialization slice, IsValid and exact-key helpers."""
from pathlib import Path

def method(s,marker):
 start=s.index(marker);opening=s.index('{',start);depth=1;i=opening+1
 while depth:
  depth+=(s[i]=='{')-(s[i]=='}');i+=1
 return s[start:i]
base=Path('legacy/decompiled')
source=(base/'PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemContents.cs').read_text(encoding='utf-8-sig')
start=source.index('equipItemInfoDic = (from x');end=source.index('if (key.StartsWith("quality_random_box"))',start)
slice=source[start:end]+'}\n'
for text,phase in [('equipItemInfoDic =',1),('itemClassInfoDic =',2),('itemClassExtraInfoDic =',3),('bool flag =',4)]:slice=slice.replace(text,'Case.Phase='+str(phase)+'; '+text,1)
slice=slice.replace('ServiceCore.FeatureMatrix.IsEnable','Case.Enabled')
ext=(base/'PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemContentsExtension.cs').read_text(encoding='utf-8-sig')
valid=method(ext,'public static bool IsValid(this ItemClassInfo');equip=method(ext,'public static EquipItemInfo GetEquipItemInfo(this ItemClassInfo')
helper=method((base/'ServiceCore-be65d59d2385/ServiceCore.HeroesContents/IEnumerableExtension.cs').read_text(encoding='utf-8-sig'),'public static Dictionary<K, T> ToDictionary_OverwriteDuplicated')
lookup=method((base/'Utility-37359981d509/Utility/DictionaryExtension.cs').read_text(encoding='utf-8-sig'),'public static V TryGetValue<T, V>(this IDictionary<T, V>')
feature=(base/'ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs').read_text(encoding='utf-8-sig');enabled=method(feature,'public static bool IsEnable(string featureName)')
generated='using System;using System.Collections.Generic;using System.Linq;\npublic static class ItemContents { public static Dictionary<string,EquipItemInfo> equipItemInfoDic; public static Dictionary<string,object> itemStatDic; public static Dictionary<string,ItemClassInfo> itemClassInfoDic; public static Dictionary<string,ItemClassExtraInfo> itemClassExtraInfoDic; public static void LoadProjection(){'+slice+'}}\npublic static class Extension {'+valid+equip+helper+lookup+' public static object GetItemStat(this EquipItemInfo info){return null;} }\nnamespace ServiceCore { public static class FeatureMatrix { public static string CurrentVer {get {return Case.Features["CurrentVer"];}} public static string GetString(string key){string value;return Case.Features.TryGetValue(key,out value)?value:"";}'+enabled+'}}'
Path('.cache/framework/ItemCatalogRecovered.cs').write_text(generated,encoding='utf-8',newline='\n')
print('Catalog slice recovered; item-stat body and rate/remaining table initialization deliberately excluded')
