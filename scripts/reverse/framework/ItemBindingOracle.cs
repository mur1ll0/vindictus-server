using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;

public sealed class ItemClassInfo { public string ItemClass, Bind, TradeOnBind; public bool equippable; }
public sealed class ItemAttributeElement { public string Value; public int Arg; }
public sealed class EnchantInfo { public int EnchantLevel; }
public static class ItemClassConst { public const string Gold="gold", EnchantScroll="enchant_scroll"; }
public static class AttributeClass { public static Array GemstoneTypes=Enum.GetValues(typeof(GemstoneType)); }
public static class ItemContents {
    public static EnchantInfo GetEnchantInfo(string key) {
        Case.Calls.Add("enchant:"+(key??"<null>"));
        if(key=="study_low")return new EnchantInfo{EnchantLevel=6};
        if(key=="study_high")return new EnchantInfo{EnchantLevel=7};
        return null;
    }
}
namespace ServiceCore { public static class FeatureMatrix {
    public static bool IsEnable(string key) {
        Case.Calls.Add("feature:"+(key??"<null>"));
        if(key=="error")throw new InvalidOperationException();
        if(key=="Macro_EnchantTradeRestriction")return Case.Macro;
        return key==null || key=="" || key=="on";
    }
    public static int GetInteger(string key) {
        Case.Calls.Add("slots");
        if(Case.SlotError)throw new InvalidOperationException();
        return Case.Slots;
    }
}}
public static class Case {
    public static List<string> Calls=new List<string>();
    public static int Slots;
    public static bool Macro, SlotError;
    public static string Hex(string text) { return text==null ? null : BitConverter.ToString(Encoding.Unicode.GetBytes(text)).Replace("-","").ToLowerInvariant(); }
    public static IDictionary<string,object> Attribute(string name,string value,int arg=0,bool nullElement=false) { return new Dictionary<string,object>{{"name",Hex(name)},{"value",Hex(value)},{"arg",arg},{"null_element",nullElement}}; }
    public static object Run(string mode, string cls, bool equip, bool bound, string bind,string trade, bool macro,int slots,bool slotError, params IDictionary<string,object>[] attrs) {
        Calls.Clear(); Slots=slots; Macro=macro; SlotError=slotError;
        var dictionary=new Dictionary<string,ItemAttributeElement>();
        foreach(var a in attrs) {
            string name=Encoding.Unicode.GetString(Unhex((string)a["name"]));
            dictionary.Add(name,(bool)a["null_element"] ? null : new ItemAttributeElement {Value=a["value"]==null ? null : Encoding.Unicode.GetString(Unhex((string)a["value"])),Arg=(int)a["arg"]});
        }
        var info=cls==null ? null : new ItemClassInfo {ItemClass=cls,equippable=equip,Bind=bind,TradeOnBind=trade};
        object result=null; string error=null;
        try { result=mode=="bind" ? ItemGenerator.CheckCharacterBind(bound,info,dictionary) : Item.CheckHighLevelEquipment(info,dictionary); }
        catch(FormatException){error="IntegerFormat";}
        catch(OverflowException){error="IntegerOverflow";}
        catch(NullReferenceException){error="MissingEnchant";}
        catch(IndexOutOfRangeException){error="GemstoneRange";}
        catch(InvalidOperationException){error="Dependency";}
        return new {mode=mode,cls_hex=Hex(cls),equippable=equip,bound=bound,bind=bind,trade=trade,macro=macro,slots=slots,slot_error=slotError,attrs=attrs,result=result,error=error,calls=Calls.ToArray()};
    }
    public static byte[] Unhex(string h) {var b=new byte[h.Length/2];for(int i=0;i<b.Length;i++)b[i]=byte.Parse(h.Substring(i*2,2),NumberStyles.HexNumber);return b;}
}
public static class Program {
    public static void Main() {
        Thread.CurrentThread.CurrentCulture=new CultureInfo("en-US",false);
        Thread.CurrentThread.CurrentUICulture=new CultureInfo("en-US",false);
        var cases=new List<object>();
        foreach(string cls in new[]{"study_equipment","gold","silver_coin","enchant_scroll_study","Enchant_scroll_study","SUCCUBUS_WING_UPPER_CASH","succubus_wing_upper_cash",""})
        foreach(bool equip in new[]{false,true})
        foreach(bool bound in new[]{false,true})
        foreach(string bind in new[]{null,"","off","on","error"})
            cases.Add(Case.Run("bind",cls,equip,bound,bind,"off",false,4,false));
        foreach(string cls in new[]{null,"study_equipment","gold","enchant_scroll_study"})
        foreach(string trade in new[]{null,"","off","on","error"})
        foreach(bool macro in new[]{false,true})
            cases.Add(Case.Run("bind",cls,true,false,"off",trade,macro,4,false));
        foreach(string value in new[]{null,"","0","8","9","10","-1","+9"," 9 ","\t9\r\n","9\0","9\0 ","2147483647","2147483648","-2147483648","-2147483649","999999999999x","999999999999","+","1.0","９","\u00a09\u00a0","+ 9","9\0\0","00000000000000000000009"})
        foreach(string mode in new[]{"bind","high"}) {
            cases.Add(Case.Run(mode,"study_equipment",true,false,"off","off",false,4,false,Case.Attribute("ENHANCE",value),Case.Attribute("LOOK","study")));
            cases.Add(Case.Run(mode,"study_equipment",true,false,"off","off",false,4,false,Case.Attribute("ENHANCE",value)));
        }
        foreach(int level in new[]{int.MinValue,0,4,5,int.MaxValue})
            cases.Add(Case.Run("high","study_equipment",true,false,"off","off",false,4,false,Case.Attribute("QUALITY","ignored",level),Case.Attribute("PREFIX","study_missing")));
        foreach(string prefix in new[]{null,"study_missing","study_low","study_high","STUDY_HIGH",""})
        foreach(string suffix in new[]{"study_low","study_high","study_missing"})
            cases.Add(Case.Run("high","study_equipment",true,false,"off","off",false,4,false,Case.Attribute("PREFIX",prefix),Case.Attribute("SUFFIX",suffix)));
        foreach(int slots in new[]{int.MinValue,-1,0,1,2,3,4,5,int.MaxValue})
        foreach(string name in new[]{"study_absent","GS_0","GS_1","GS_2","GS_3","GS_4","gs_0"})
        foreach(bool nullElement in new[]{false,true})
            cases.Add(Case.Run("high","study_equipment",true,false,"off","off",false,slots,false,Case.Attribute(name,"unused",0,nullElement)));
        foreach(string name in new[]{"LOOK","ENHANCE","QUALITY","PREFIX","SUFFIX"})
            cases.Add(Case.Run("high","study_equipment",true,false,"off","off",false,0,false,Case.Attribute(name,null,0,true)));
        cases.Add(Case.Run("high","study_equipment",true,false,"off","off",false,4,true));
        cases.Add(Case.Run("high","study_equipment",false,false,"error","error",false,4,true,Case.Attribute("ENHANCE","bad")));
        cases.Add(Case.Run("high","SUCCUBUS_WING_UPPER_CASH",true,false,"error","error",false,4,true,Case.Attribute("ENHANCE","bad")));
        cases.Add(Case.Run("high","study_equipment",true,false,"off","off",false,4,true,Case.Attribute("QUALITY","x",5),Case.Attribute("PREFIX","study_missing")));
        cases.Add(Case.Run("high","study_equipment",true,false,"off","off",false,4,true,Case.Attribute("ENHANCE","9"),Case.Attribute("PREFIX","study_missing")));
        string prefixClass="enchant_scroll";
        for(char c=(char)0x20;c<=(char)0x7e;c++) {
            for(int i=0;i<prefixClass.Length;i++) {
                var changed=prefixClass.ToCharArray(); changed[i]=c;
                cases.Add(Case.Run("bind",new string(changed),true,false,"off","off",true,0,false));
            }
            cases.Add(Case.Run("bind",c+prefixClass,true,false,"off","off",true,0,false));
            cases.Add(Case.Run("bind",prefixClass+c,true,false,"off","off",true,0,false));
        }
        string wing="succubus_wing_upper_cash";
        for(int i=0;i<wing.Length;i++) {
            var changed=wing.ToCharArray();changed[i]=char.ToUpperInvariant(changed[i]);
            cases.Add(Case.Run("high",new string(changed),true,false,"off","off",false,4,true,Case.Attribute("ENHANCE","bad")));
        }
        var unsupported=new List<object>();
        foreach(string cls in new[]{"enchant\0_scroll_study","enchánt_scroll_study","İtem","süccubus_wing_upper_cash","study\u007f","study\t","study\ud800"}) {
            unsupported.Add(Case.Run("bind",cls,true,false,"off","off",false,4,false));
            cases.Add(Case.Run("bind",cls,true,true,"error","error",false,4,true));
            cases.Add(Case.Run("bind",cls,false,false,"error","error",false,4,true));
        }
        Console.Write(new JavaScriptSerializer{MaxJsonLength=4000000}.Serialize(new {architecture=IntPtr.Size==8?"x64":"x86",culture="en-US",gemstone_values=Enum.GetValues(typeof(GemstoneType)),cases=cases,unsupported=unsupported}));
    }
}
