using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Runtime.Serialization;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.ItemServiceOperations;
using ServiceCore.CharacterServiceOperations;
class ItemSyncWireOracle {
 static int[] Body(Packet p) { return p.Array.Skip(p.Offset+p.BodyOffset).Take(p.Length).Select(b=>(int)b).ToArray(); }
 static object Capture(object v) {
  if(v==null)return null;
  if(v is string || v.GetType().IsPrimitive)return v;
  if(v is IDictionary){var rows=new List<object>();foreach(DictionaryEntry x in (IDictionary)v) rows.Add(new {key=Capture(x.Key),value=Capture(x.Value)});return rows;}
  if(v is IEnumerable)return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
  var props=v.GetType().GetProperties().Where(p=>p.CanRead).ToArray();
  if(props.Length==0)return v.GetType().GetFields(BindingFlags.Instance|BindingFlags.NonPublic|BindingFlags.Public).ToDictionary(f=>f.Name,f=>Capture(f.GetValue(v)));
  return v.GetType().GetProperties().Where(p=>p.CanRead && p.Name!="Color1" && p.Name!="Color2" && p.Name!="Color3").ToDictionary(p=>p.Name,p=>Capture(p.GetValue(v,null)));
 }
 static T Obj<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetProperty((string)fields[i]).SetValue(obj,fields[i+1],null);
  return obj;
 }
 static SlotInfo Slot(int seed) {
  return Obj<SlotInfo>("ItemID",(long)seed,"ItemClass",seed==0?null:"synthetic_é_装備","Num",seed,"Tab",seed,"Slot",seed,"IsExpireable",seed!=0,"ExpireDateTimeDiff",(long)seed,"ExpireDateTimeTick",long.MaxValue,"Colors",new[]{seed,-1,int.MaxValue},"Durability",seed,"MaxDurability",int.MaxValue,"MaxDurabilityBonus",int.MinValue,"Tradable",true,"ExpanderName","","Attributes",new List<ItemAttributeElement>{null,new ItemAttributeElement("TEST","値",seed,int.MinValue)},"PrefixEnchantStatus",new Dictionary<int,bool>{{-1,true},{7,false}},"SuffixEnchantStatus",new Dictionary<int,bool>(),"MinPrice",int.MinValue,"MaxPrice",int.MaxValue,"AvgPrice",seed,"MinUpTimeDateTimeDiff",long.MinValue);
 }
 static T Priv<T>(string field,object value) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  typeof(T).GetField(field,BindingFlags.Instance|BindingFlags.NonPublic).SetValue(obj,value);
  return obj;
 }
 static T Fld<T>(params object[] fields) where T:class {
  var obj=(T)FormatterServices.GetUninitializedObject(typeof(T));
  for(int i=0;i<fields.Length;i+=2)typeof(T).GetField((string)fields[i],BindingFlags.Instance|BindingFlags.NonPublic|BindingFlags.Public).SetValue(obj,fields[i+1]);
  return obj;
 }
 static CostumeInfo Costume(int seed) {
  var c=new CostumeInfo();
  c.CostumeTypeInfo=new Dictionary<int,int>{{0,seed},{int.MaxValue,-1}};
  c.ColorInfo=new Dictionary<int,int>{{int.MinValue,seed}};
  c.AvatarInfo=new Dictionary<int,bool>{{1,true},{2,false}};
  c.AvatarHideInfo=null;
  c.PollutionInfo=new Dictionary<int,byte>{{3,255}};
  c.EffectInfo=new Dictionary<int,int>();
  c.DecorationInfo=new Dictionary<int,int>{{4,seed}};
  c.DecorationColorInfo=new Dictionary<int,int>{{5,-1}};
  c.Shineness=seed;c.Height=int.MinValue;c.Bust=int.MaxValue;c.PaintingPosX=1;c.PaintingPosY=-1;c.PaintingRotation=seed;c.PaintingSize=2;
  c.HideHeadCostume=seed!=0;c.CafeType=seed;c.IsReturn=seed==0;c.VIPCode=seed;c.BodyPaintingPosX=3;c.BodyPaintingPosY=4;c.BodyPaintingRotation=5;c.BodyPaintingSize=6;c.BodyPaintingSide=7;c.BodyPaintingClip=8;c.BodyPaintingMode=9;
  c.BodyShapeInfo=new Dictionary<int,float>{{0,1.5f},{1,float.NaN},{2,-0.0f}};
  c.TownEffect=seed;
  return c;
 }
    static void AddCases<T>(string kind, int category, Guid expectedGuid, IEnumerable<T> values, List<object> cases) where T : class {
        if (typeof(T).GUID != expectedGuid) {
            throw new InvalidOperationException("GUID mismatch for " + typeof(T).FullName);
        }
        var items = values.ToArray();
        var generic = items.Select(v => SerializeWriter.ToBinary<T>(v)).ToArray();
        ClassInfo<T>.CategoryId = category;
        for (int n = 0; n < items.Length; n++) {
            var typed = SerializeWriter.ToBinary<T>(items[n]);
            foreach (string mode in new[] { "unregistered-generic", "registered-object", "registered-typed" }) {
                var packet = mode == "unregistered-generic" ? generic[n]
                    : mode == "registered-object" ? SerializeWriter.ToBinary((object)items[n])
                    : typed;
                T decoded;
                SerializeReader.FromBinary<T>(packet, out decoded);
                var reencoded = SerializeWriter.ToBinary<T>(decoded);
                if (!Body(reencoded).SequenceEqual(Body(typed))) {
                    throw new InvalidOperationException("Roundtrip mismatch for " + kind + " case " + n);
                }
                // Verify MessageAnalyzer (unencrypted channel transport) frames and deframes packet.Bytes identically.
                var analyzer = new MessageAnalyzer();
                var raw = packet.Bytes;
                var copy = new byte[raw.Count];
                Buffer.BlockCopy(raw.Array, raw.Offset, copy, 0, raw.Count);
                analyzer.Add(new ArraySegment<byte>(copy));
                var analyzed = analyzer.ToArray();
                if (analyzed.Length != 1) {
                    throw new InvalidOperationException("MessageAnalyzer count mismatch");
                }
                var framed = new Packet(analyzed[0]);
                if (framed.CategoryId != packet.CategoryId || !Body(framed).SequenceEqual(Body(packet))) {
                    throw new InvalidOperationException("MessageAnalyzer payload mismatch");
                }
                cases.Add(new {
                    kind,
                    mode,
                    category = packet.CategoryId,
                    body = Body(packet),
                    typed_body = Body(typed),
                    is_null = decoded == null,
                    fields = Capture(decoded)
                });
            }
        }
    }

 static void Main(){
  var cases=new List<object>();
  var updates=new List<UpdateInventoryInfoMessage>{null,Priv<UpdateInventoryInfoMessage>("slotInfos",null),new UpdateInventoryInfoMessage(new List<SlotInfo>()),new UpdateInventoryInfoMessage(new List<SlotInfo>{null}),new UpdateInventoryInfoMessage(new List<SlotInfo>{Obj<SlotInfo>()})};
  foreach(int seed in new[]{0,-1,1,int.MinValue,int.MaxValue})updates.Add(new UpdateInventoryInfoMessage(new List<SlotInfo>{Slot(seed),null,Slot(seed^1)}));
  AddCases("update_inventory_info",617,new Guid("abacd3bc-1a84-3570-a62c-f1b0007675fb"),updates,cases);
  var fails=new List<ItemFailMessage>{null,new ItemFailMessage(null),new ItemFailMessage(Obj<HeroesString>()),new ItemFailMessage(new HeroesString("GameUI_Heroes_EquipItemFail_System")),new ItemFailMessage(new HeroesString("GameUI_Heroes_ConstraintFail_LowLevel",new string[]{null,"","値"})),new ItemFailMessage(new HeroesString(null,new string[0]))};
  AddCases("item_fail",358,new Guid("2f756b90-8fba-38b8-bcb2-a51f0f132502"),fails,cases);
  var costumes=new List<CostumeUpdateMessage>{null,new CostumeUpdateMessage(null),new CostumeUpdateMessage(new CostumeInfo())};
  foreach(int seed in new[]{0,-1,1,int.MinValue,int.MaxValue})costumes.Add(new CostumeUpdateMessage(Costume(seed)));
  var moves=new List<MoveInventoryItemMessage>{null};
  foreach(long id in new[]{long.MinValue,-1L,0L,long.MaxValue})foreach(byte st in new byte[]{0,6,100,255})foreach(int sl in new[]{int.MinValue,-1,0,47,48,int.MaxValue}){var m=new MoveInventoryItemMessage();m.ItemID=id;m.Storage=st;m.Target=sl;moves.Add(m);}
  AddCases("move_inventory_item",502,new Guid("7c957f24-e8d7-338f-a93c-97bc6903f320"),moves,cases);
  var bundles=new List<EquipBundleMessage>{null};
  foreach(string c in new[]{null,"","life_potion_lvl0","skill_synthetic"})foreach(int q in new[]{int.MinValue,-1,0,3,100,119,int.MaxValue})bundles.Add(new EquipBundleMessage(c,q));
  AddCases("equip_bundle",456,new Guid("6652c42f-bf01-3da0-9415-f390b085e456"),bundles,cases);
  var quick=new List<QuickSlotInfoMessage>{null,new QuickSlotInfoMessage(null),new QuickSlotInfoMessage(new QuickSlotInfo(null)),new QuickSlotInfoMessage(new QuickSlotInfo(new Dictionary<int,string>())),new QuickSlotInfoMessage(new QuickSlotInfo(new Dictionary<int,string>{{3,""},{100,"life_potion_lvl0"},{101,null},{int.MinValue,"synthetic"}}))};
  AddCases("quick_slot_info",551,new Guid("edc4290e-196c-3f17-903f-d0f3317e47d9"),quick,cases);
  AddCases("query_ap",526,new Guid("f061ae98-f5c8-333d-82ca-3b217e268120"),new[]{null,new QueryAPMessage()},cases);
  var aps=new List<APMessage>{null};
  foreach(int a in new[]{int.MinValue,-1,0,500,int.MaxValue})foreach(long t in new[]{long.MinValue,-1L,0L,long.MaxValue})aps.Add(new APMessage(a,a==0?-1:1000,t,a==500?-1:60));
  AddCases("ap",229,new Guid("3d59ac68-6b3c-3f92-968e-b569b973f065"),aps,cases);
  var uses=new List<UseInventoryItemMessage>{null};
  foreach(long id in new[]{long.MinValue,-1L,0L,long.MaxValue})foreach(long tid in new[]{-1L,0L,long.MaxValue})uses.Add(new UseInventoryItemMessage(id,tid));
  AddCases("use_inventory_item",629,new Guid("c182cb18-7469-3390-a0e7-8eba31d6e45d"),uses,cases);
  AddCases("use_inventory_item_ok",627,new Guid("451d459f-bb17-325c-bb10-d608f03f4c3f"),new[]{null,Obj<UseInventoryItemOkMessage>(),Obj<UseInventoryItemOkMessage>("UseItemClass",""),Obj<UseInventoryItemOkMessage>("UseItemClass","capsule_ap50")},cases);
  AddCases("query_skill_list",540,new Guid("860e6484-d1b5-34a9-9412-70807bbbf79c"),new[]{null,new QuerySkillListMessage()},cases);
  var lists=new List<SkillListMessage>{null,new SkillListMessage(null,0,null,false),new SkillListMessage("",0,new List<BriefSkillInfo>(),true)};
  foreach(int seed in new[]{int.MinValue,-1,0,1,30,int.MaxValue}){
   var b=new BriefSkillInfo{SkillID=seed==0?null:"skill_synthetic",BaseRank=seed,FinalRank=seed,RequiredAP=-1,IsLocked=(byte)(seed&255),CanStartTraining=(byte)(seed==30?1:0),CurrentAP=seed,ResetSkillAP=-1,UntrainSkillAP=seed};
   if(seed==1)b.Enhances=null;
   if(seed==30)b.Enhances.Add(new BriefSkillEnhance{GroupKey=seed,IndexKey=-1,Type=255,ReduceDurability=int.MinValue,MaxDurabilityBonus=int.MaxValue});
   lists.Add(new SkillListMessage(seed==-1?null:"skill_learning",seed,new List<BriefSkillInfo>{b,null},seed%2==0));
  }
  AddCases("skill_list",583,new Guid("dd3b94a9-64da-381a-b2e7-0fad58835746"),lists,cases);
  var learns=new List<LearnSkillMessage>{null};
  foreach(string sk in new[]{null,"","skill_fiona_guard"})foreach(int ap in new[]{int.MinValue,-1,0,30,int.MaxValue})learns.Add(new LearnSkillMessage(sk,ap));
  AddCases("learn_skill",475,new Guid("3ec00564-95e3-3507-b9dc-a5a185215b19"),learns,cases);
  var completions=new List<SkillCompletionMessage>{null};
  foreach(string sk in new[]{null,"skill_fiona_guard"})foreach(int r in new[]{int.MinValue,0,2,int.MaxValue})completions.Add(new SkillCompletionMessage(sk,r));
  AddCases("skill_completion",582,new Guid("2b5614fc-8830-3aec-a03f-c33c270de9f1"),completions,cases);
  var commons=new List<QueryCharacterCommonInfoMessage>{null};
  foreach(long q in new[]{long.MinValue,0L,7L,long.MaxValue})foreach(long c in new[]{-1L,1L,long.MaxValue})commons.Add(Obj<QueryCharacterCommonInfoMessage>("QueryID",q,"CID",c));
  AddCases("query_common_info",527,new Guid("c9cda3f6-8b67-3c69-ba31-ba2fe827fd7b"),commons,cases);
  var news=new List<LearnNewSkillMessage>{null};
  foreach(string sk in new[]{null,"","skill_fiona_guard"})news.Add(new LearnNewSkillMessage(sk));
  AddCases("learn_new_skill",340,new Guid("7fbd47e1-96a3-3472-9e71-f36d90f7294f"),news,cases);
  var results=new List<LearnNewSkillResultMessage>{null};
  foreach(LearnNewSkillResultMessage.LearnNewSkillResult r in Enum.GetValues(typeof(LearnNewSkillResultMessage.LearnNewSkillResult)))results.Add(new LearnNewSkillResultMessage(r));
  AddCases("learn_new_skill_result",341,new Guid("0430910f-e20e-3736-850d-b619e836f14f"),results,cases);
  AddCases("query_npc_list",533,new Guid("6d029dfe-89e5-32d0-aa79-27d298e3b87c"),new[]{null,new QueryNpcListMessage()},cases);
  var stories=new List<StoryInfo>{new StoryInfo(null,false),null,new StoryInfo("story_synthetic",true)};
  var npcs=new List<NpcInfo>{new NpcInfo(null,null,null),null,new NpcInfo("FERGHUS","",new List<StoryInfo>()),new NpcInfo("ARIEL","S2EP1&&SpiritInjection",stories)};
  var buildings=new List<BuildingInfo>{new BuildingInfo(null,null),null,new BuildingInfo("t01_bs",npcs),new BuildingInfo("t01_inn",new List<NpcInfo>())};
  AddCases("npc_list",507,new Guid("35be098a-89fe-3185-a8f3-1cf91f5c07c1"),new[]{null,new NpcListMessage(null),new NpcListMessage(new List<BuildingInfo>()),new NpcListMessage(buildings)},cases);
  var talks=new List<QueryNpcTalkMessage>{null};
  foreach(string loc in new[]{null,"","t01_bs"})foreach(string npc in new[]{null,"FERGHUS"})foreach(string sl in new[]{null,"default"})foreach(string cmd in new[]{null,"greeting"})talks.Add(new QueryNpcTalkMessage(loc,npc,sl,cmd));
  AddCases("query_npc_talk",534,new Guid("986c6c40-c9b6-3aed-9303-0b5679aa8f53"),talks,cases);
  var entities=new List<NpcTalkEntity>{new NpcTalkEntity("talk","HEROES_STORYLINE_SYNTHETIC"),null,new NpcTalkEntity("change_npc",null,"x"),Obj<NpcTalkEntity>("Command",null,"Arg1",null,"Arg2",null)};
  AddCases("npc_talk",508,new Guid("859ce7b4-cfd1-3695-9a73-b5c094d7085e"),new[]{null,new NpcTalkMessage(null),new NpcTalkMessage(new List<NpcTalkEntity>()),new NpcTalkMessage(entities)},cases);
  var buttons=new List<SelectButtonMessage>{null};
  foreach(int b in new[]{int.MinValue,-1,0,3,int.MaxValue})buttons.Add(new SelectButtonMessage(b));
  AddCases("select_button",564,new Guid("d5dda62d-44c1-3c6a-8906-72164e564921"),buttons,cases);
  var hides=new List<HideCostumeMessage>{null,new HideCostumeMessage(false),new HideCostumeMessage(true)};
  foreach(bool h in new[]{false,true})foreach(int part in new[]{int.MinValue,-1,0,54,58,int.MaxValue})foreach(int st in new[]{int.MinValue,-1,0,1,int.MaxValue})hides.Add(Obj<HideCostumeMessage>("HideHead",h,"AvatarPart",part,"AvatarState",st));
  AddCases("hide_costume",414,new Guid("80e685d2-1ace-3cf5-90aa-7fda984d169d"),hides,cases);
  AddCases("set_story_line_status",574,new Guid("92e441fa-adce-304e-aeb2-a77fb76d2900"),new[]{null,new SetStoryLineStatusMessage(null,0),new SetStoryLineStatusMessage("GuidePlay_1_SkillTrain",1),new SetStoryLineStatusMessage("",255)},cases);
  AddCases("execute_npc_server_command",459,new Guid("048fdbb2-e452-3c5f-b0b2-cfc80994e365"),new[]{null,new ExecuteNpcServerCommandMessage()},cases);
  var briefs=new List<BriefStoryLineInfo>{new BriefStoryLineInfo(null,null,255),null,new BriefStoryLineInfo("GuidePlay_1_SkillTrain","p0",1,"goal_text"),new BriefStoryLineInfo("S0NT_99_CutScene","",0,null)};
  AddCases("story_lines",598,new Guid("5498b052-d119-30e2-8224-1e2b990d4ef8"),new[]{null,new StoryLinesMessage(null,0),new StoryLinesMessage(new List<BriefStoryLineInfo>(),int.MinValue),new StoryLinesMessage(briefs,3)},cases);
  var buys=new List<BuyItemMessage>{null};
  foreach(string sh in new[]{null,"","FERGHUS_Shop"})foreach(int o in new[]{-1,0,7})foreach(int c in new[]{int.MinValue,0,1,99})buys.Add(Obj<BuyItemMessage>("ShopID",sh,"Order",o,"Count",c));
  AddCases("buy_item",234,new Guid("2f52e151-33c1-34dc-bd6e-9535c35ab918"),buys,cases);
  var shopCrafts=new List<RequestCraftMessage>{null};
  foreach(string sh in new[]{null,"","FERGHUS_Craft"})foreach(int o in new[]{-1,0,15})foreach(int c in new[]{int.MinValue,0,1,99})shopCrafts.Add(Obj<RequestCraftMessage>("ShopID",sh,"Order",o,"Count",c));
  AddCases("request_craft",558,new Guid("7ba36e6f-5935-30d7-b5a6-7bbcda0acce7"),shopCrafts,cases);
  AddCases("query_cash_shop_product_list",242,new Guid("c60512f1-a197-3109-a28b-f89c6003d9fb"),new[]{null,Obj<QueryCashShopProductListMessage>()},cases);
  var category=Fld<CashShopCategoryListElement>("CategoryNo",105,"CategoryName","AVATAR_TUNIC_é","ParentCategoryNo",int.MinValue,"DisplayNo",-1);
  AddCases("cash_shop_category_list",244,new Guid("d5d8393b-80ea-36fb-a724-0dc9e66c1b32"),new[]{null,new CashShopCategoryListMessage(null),new CashShopCategoryListMessage(new List<CashShopCategoryListElement>()),new CashShopCategoryListMessage(new List<CashShopCategoryListElement>{category,null,Fld<CashShopCategoryListElement>("CategoryNo",int.MaxValue)})},cases);
  var product=Fld<CashShopProductListElement>("ProductNo",int.MaxValue,"ProductExpire",(short)-1,"ProductPieces",short.MaxValue,"ProductID","avatar_tunic_é","ProductGUID","00000000-0000-0000-0000-000000000001","PaymentType",13001,"ProductType","","SalePrice",1500,"CategoryNo",105,"Status",1);
  AddCases("cash_shop_product_list",243,new Guid("43148f51-9ebf-3fb3-a2ff-ceb7fe3503f2"),new[]{null,new CashShopProductListMessage(null),new CashShopProductListMessage(new List<CashShopProductListElement>{product,null,Fld<CashShopProductListElement>("ProductNo",int.MinValue,"ProductExpire",short.MinValue,"Status",-1)})},cases);
  AddCases("query_cash_shop_balance",238,new Guid("1979a6ad-f975-3268-9ebf-19875d68756b"),new[]{null,Obj<QueryCashShopBalanceMessage>()},cases);
  var balances=new List<CashShopBalanceMessage>{null};
  foreach(int b in new[]{int.MinValue,0,1000000,int.MaxValue})balances.Add(new CashShopBalanceMessage(b,b==0?0:-1));
  AddCases("cash_shop_balance",239,new Guid("b5b39940-43ec-33ec-aa7b-3f786d4bd5ac"),balances,cases);
  AddCases("query_cash_shop_inventory",240,new Guid("12671796-6f21-3148-8097-e5f33dd30899"),new[]{null,Obj<QueryCashShopInventoryMessage>()},cases);
  var order=Obj<CashShopInventoryElement>("OrderNo",7,"ProductNo",int.MaxValue,"ItemClass","avatar_tunic","ItemClassEx","avatar_tunic[X:1]","Count",short.MaxValue,"r","-1","g",null,"b","値","Expire",(short)-1,"IsGift",true,"SenderMessage","","RemainQuantity",short.MinValue);
  AddCases("cash_shop_inventory",241,new Guid("c99ca6d7-9067-380e-9b93-b9c3cfeab051"),new[]{null,Obj<CashShopInventoryMessage>(),new CashShopInventoryMessage(new List<CashShopInventoryElement>{order,null,Obj<CashShopInventoryElement>()})},cases);
  var pickups=new List<QueryCashShopItemPickUpMessage>{null};
  foreach(int o in new[]{int.MinValue,0,7})foreach(short q in new short[]{short.MinValue,0,1,short.MaxValue})pickups.Add(new QueryCashShopItemPickUpMessage(o,o+1,q));
  AddCases("query_cash_shop_item_pick_up",247,new Guid("ea22129f-5ff5-3a25-934c-764318735384"),pickups,cases);
  var purchases=new List<QueryCashShopPurchaseItemMessage>{null,Fld<QueryCashShopPurchaseItemMessage>()};
  foreach(short q in new short[]{short.MinValue,1,short.MaxValue})purchases.Add(Fld<QueryCashShopPurchaseItemMessage>("ProductNo",(int)q*3,"Quantity",q,"Attribute0","1","Attribute1",null,"Attribute2","","Attribute3","値","Attribute4","x","IsForCommonInven",q==1));
  AddCases("query_cash_shop_purchase_item",248,new Guid("52d58673-c69c-3976-9e4b-b57e2d9adc9e"),purchases,cases);
  AddCases("cash_shop_fail",250,new Guid("6a257038-7e65-3370-886d-bb0c30c7e1c8"),new[]{null,new CashShopFailMessage(0),new CashShopFailMessage(int.MinValue),new CashShopFailMessage(255)},cases);
  AddCases("wish_list_select",213,new Guid("97389473-f994-3d1d-aaa8-df5138bdffc6"),new[]{null,new WishListSelectMessage(long.MinValue),new WishListSelectMessage(0),new WishListSelectMessage(long.MaxValue)},cases);
  AddCases("wish_list_select_response",214,new Guid("643c7092-008e-3780-9e30-faa5fc68b1b5"),new[]{null,new WishListSelectResponseMessage(),new WishListSelectResponseMessage((WishListResult)1,7,new List<WishItemInfo>{new WishItemInfo(7,int.MaxValue,"値"),null,new WishItemInfo(long.MinValue,0,null)}),new WishListSelectResponseMessage((WishListResult)(-10),long.MaxValue,new List<WishItemInfo>())},cases);
  AddCases("wish_list_insert",212,new Guid("a522e7d8-cc68-346c-907a-b11cd6630452"),new[]{null,new WishListInsertMessage(1,null),new WishListInsertMessage(long.MaxValue,new List<int>{int.MinValue,0,int.MaxValue})},cases);
  AddCases("wish_list_insert_response",211,new Guid("a48093ce-a0b9-3bcd-a52b-6738c45cf30d"),new[]{null,new WishListInsertResponseMessage((WishListResult)1,7),new WishListInsertResponseMessage((WishListResult)(-9),long.MinValue)},cases);
  AddCases("wish_list_delete",209,new Guid("c4587129-aa0e-3a5d-a47d-6e17ce2550a7"),new[]{null,new WishListDeleteMessage(1,null),new WishListDeleteMessage(long.MinValue,new List<int>{5})},cases);
  AddCases("wish_list_delete_response",210,new Guid("a72f021b-568b-3ff6-8400-3c6a64c3c2bc"),new[]{null,new WishListDeleteResponseMessage((WishListResult)2,0)},cases);
  var beauty=Obj<BeautyShopCustomizeMessage>("CustomizeItems",new List<CustomizeItemRequestInfo>{new CustomizeItemRequestInfo("fiona_hair_01","HAIR",-1,5,int.MaxValue,-1,"139",-1L),new CustomizeItemRequestInfo(null,null,0,0,0,30,"coupon_hair_値",long.MaxValue)},"PaintingPosX",1,"PaintingPosY",-2,"PaintingSize",3,"PaintingRotation",int.MinValue,"payment",0,"BodyPaintingPosX",4,"BodyPaintingPosY",5,"BodyPaintingSize",6,"BodyPaintingRotation",7,"BodyPaintingSide",8,"BodyPaintingClip",9,"BodyPaintingMode",10,"HeightValue",11,"BustValue",12,"ShinenessValue",13,"SkinColor",14,"EyeColor",15,"EyebrowItemClass","fiona_brow01","EyebrowColor",16,"LookChangeItemClass",null,"LookChangeDuration",-1,"LookChangePrice","","BodyShapeInfo",new Dictionary<int,float>{{0,0.5f},{int.MaxValue,-1.25f}});
  AddCases("beauty_shop_customize",233,new Guid("31e4e1d4-592f-3f80-b094-27cc6a284f97"),new[]{null,Obj<BeautyShopCustomizeMessage>(),Obj<BeautyShopCustomizeMessage>("CustomizeItems",new List<CustomizeItemRequestInfo>{new CustomizeItemRequestInfo("fiona_hair_01","HAIR",-1,5,7,-1,"139",-1L)}),Obj<BeautyShopCustomizeMessage>("CustomizeItems",new List<CustomizeItemRequestInfo>(),"payment",2,"BodyShapeInfo",new Dictionary<int,float>()),Obj<BeautyShopCustomizeMessage>("BodyShapeInfo",new Dictionary<int,float>{{0,0.5f}}),beauty},cases);
  AddCases("request_secured_operation",298,new Guid("d7b15418-06ce-3d45-8e42-08179db5f5a6"),new[]{null,Obj<RequestSecuredOperationMessage>(),Obj<RequestSecuredOperationMessage>("Operation",(SecuredOperationType)1),Obj<RequestSecuredOperationMessage>("Operation",(SecuredOperationType)2)},cases);
  AddCases("secured_operation",299,new Guid("e4c5c606-88ae-304e-9431-97df6b3c4419"),new[]{null,Obj<SecuredOperationMessage>(),Obj<SecuredOperationMessage>("Operation",(SecuredOperationType)1,"LockedTimeInSeconds",0),Obj<SecuredOperationMessage>("Operation",(SecuredOperationType)2,"LockedTimeInSeconds",int.MaxValue)},cases);
  AddCases("use_extra_storage",626,new Guid("e9cd479d-3b6b-3d9e-850e-c228d89b7aa7"),new[]{null,new UseExtraStorageMessage(0L,(byte)0),new UseExtraStorageMessage(42L,(byte)2),new UseExtraStorageMessage(long.MaxValue,byte.MaxValue)},cases);
  AddCases("buy_item_result",235,new Guid("64a08bdf-6762-3be2-9f6f-c7ff64a2b4a3"),new[]{null,new BuyItemResultMessage(BuyItemResultMessage.BuyItemResult.Failed,null,0,null,0),new BuyItemResultMessage(BuyItemResultMessage.BuyItemResult.Succeeded,"potion_hp_lvl1",10,"gold",500,3,-1),new BuyItemResultMessage(BuyItemResultMessage.BuyItemResult.Failed_UniqueConstraint,"x",int.MaxValue,"",int.MinValue,int.MinValue,int.MaxValue)},cases);
  AddCases("query_shop_item_info",167,new Guid("4088bd54-bc81-3398-8386-61bdaf5917c1"),new[]{null,Obj<QueryShopItemInfoMessage>("ShopID",null),Obj<QueryShopItemInfoMessage>("ShopID",""),Obj<QueryShopItemInfoMessage>("ShopID","FERGHUS_Shop")},cases);
  var sells=new List<SellItemMessage>{null};
  foreach(long id in new[]{long.MinValue,-1L,0L,long.MaxValue})foreach(int c in new[]{int.MinValue,0,1,int.MaxValue})sells.Add(Obj<SellItemMessage>("ItemID",id,"Count",c));
  AddCases("sell_item",566,new Guid("62669042-85a8-354c-9cab-ccae3c2becae"),sells,cases);
  AddCases("query_buyback_list",293,new Guid("2271100a-effc-3693-b75d-165507d49b79"),new[]{null,new QueryBuybackListMessage()},cases);
  var backs=new List<BuybackInfo>{new BuybackInfo(0,"potion",3,-1,-1,-1,40),null,new BuybackInfo(int.MaxValue,null,int.MinValue,1,2,3,int.MaxValue)};
  backs[2].ShopID=null;backs[2].PriceItemClass=null;
  AddCases("buyback_list_result",294,new Guid("88ce410b-fcb7-3794-ab5f-9cb25771ce3d"),new[]{null,new BuybackListResultMessage(null),new BuybackListResultMessage(new List<BuybackInfo>()),new BuybackListResultMessage(backs)},cases);
  var repairs=new List<RepairItemMessage>{null,new RepairItemMessage(null),new RepairItemMessage(new List<long>{long.MinValue,0L,long.MaxValue}){AddAllEquippedItems=true,AddAllRepairableItems=true}};
  var nullIds=new RepairItemMessage(new List<long>());nullIds.ItemIDs=null;nullIds.AddAllBrokenItems=true;repairs.Add(nullIds);
  AddCases("repair_item",424,new Guid("b83c8562-c97c-343c-b421-d3cdada26774"),repairs,cases);
  AddCases("update_story_guide",622,new Guid("4f661234-1161-3db9-8127-877ed7807435"),new[]{null,new UpdateStoryGuideMessage(null,null),new UpdateStoryGuideMessage("",""),new UpdateStoryGuideMessage("colhen.adv_guild","HEROES_STORYLINE_GUIDE")},cases);
  var vars=new List<StoryVariableInfo>{new StoryVariableInfo("GuidePlay","count",3),null,new StoryVariableInfo(null,null,int.MinValue)};
  AddCases("story_variables",599,new Guid("de76b127-f09d-369a-8008-26af3e5c8383"),new[]{null,new StoryVariablesMessage(null),new StoryVariablesMessage(new List<StoryVariableInfo>()),new StoryVariablesMessage(vars)},cases);
  AddCases("query_quest_progress",535,new Guid("5b634b00-c047-339e-a4fe-b0ef8e3b9a2e"),new[]{null,new QueryQuestProgressMessage()},cases);
  var times=new List<ClearTimeElement>{new ClearTimeElement{Difficulty=2,MemberCount=4,ClearTime=612,PlayRank=1,ClearInfo=7,IsOld=true},new ClearTimeElement()};
  var goal=new AchieveGoalInfo(10010,2,3,255,new DateTime(2020,1,1));goal.LastClearTimeDiff=int.MinValue;
  var progress=new List<QuestProgressInfo>{new QuestProgressInfo("quest_gnoll_watchman",0,0,true,true,false,true,-1,0,new List<ClearTimeElement>()),null,new QuestProgressInfo(null,int.MinValue,int.MaxValue,false,false,true,false,5,4,null),new QuestProgressInfo("quest_x",100,2,false,true,true,true,int.MaxValue,1,times)};
  AddCases("quest_progress",546,new Guid("c229a0f5-4eda-3bbc-a4be-61f3aad3f82b"),new[]{null,new QuestProgressMessage(null,null),new QuestProgressMessage(new List<QuestProgressInfo>(),new List<AchieveGoalInfo>()),new QuestProgressMessage(new List<QuestProgressInfo>{progress[0],null},null),new QuestProgressMessage(new List<QuestProgressInfo>{progress[2]},null),new QuestProgressMessage(new List<QuestProgressInfo>{progress[3]},null),new QuestProgressMessage(null,new List<AchieveGoalInfo>{goal}),new QuestProgressMessage(null,new List<AchieveGoalInfo>{null})},cases);
  var slot=new TitleSlotInfo(410017,TitleState.Known,0,"",new List<KeyValuePair<int,int>>{new KeyValuePair<int,int>(1,0),new KeyValuePair<int,int>(2,int.MinValue)},null);
  var acquired=new TitleSlotInfo(7,TitleState.Acquired,long.MaxValue,"quest_x",new List<KeyValuePair<int,int>>(),new DateTime(2030,1,2));
  AddCases("update_title",191,new Guid("bb01bf3c-8194-3f94-9480-4bf20654b7b3"),new[]{null,new UpdateTitleMessage(null),new UpdateTitleMessage(new List<TitleSlotInfo>()),new UpdateTitleMessage(new List<TitleSlotInfo>{slot,null,acquired})},cases);
  AddCases("title_list",610,new Guid("e537b12b-c3c8-3ae2-816e-df0465b4883b"),new[]{null,new TitleListMessage(null,null),new TitleListMessage(new List<TitleSlotInfo>(),new List<TitleSlotInfo>()),new TitleListMessage(new List<TitleSlotInfo>{acquired},new List<TitleSlotInfo>{slot,null,acquired})},cases);
  var enhances=new List<EnhanceItemMessage>{null};
  foreach(long id in new[]{long.MinValue,0L,long.MaxValue})foreach(string m in new[]{null,"","enhance_stone_lvl1"})foreach(string add in new[]{null,"","Enhance_Destroy_ProtectRune"})foreach(bool e in new[]{false,true})enhances.Add(Obj<EnhanceItemMessage>("ItemID",id,"Material1",m,"Material2",m==null?"enhance_elixir":null,"AdditionalMaterial",add,"IsEventEnhanceAShot",e));
  AddCases("enhance_item",354,new Guid("04804f38-5cec-31f6-885a-cc0fc33628eb"),enhances,cases);
  var rewards=new Dictionary<string,int>{{"enhance_stone_lvl1",21},{"iron_ingot",int.MinValue},{"",int.MaxValue}};
  AddCases("enhance_item_result",355,new Guid("1726cb88-3409-3a2b-8923-56e0250aab6a"),new[]{null,new EnhanceItemResultMessage(null,0,null,null,null),new EnhanceItemResultMessage("Success",long.MaxValue,"longsword_swordwind","longsword_swordwind[ENHANCE:1]",new Dictionary<string,int>()),new EnhanceItemResultMessage("Destroy",long.MinValue,"longsword_swordwind[ENHANCE:9]","",rewards),new EnhanceItemResultMessage("NoPenalty_RankDown",-1,"",null,new Dictionary<string,int>{{"x",0}})},cases);
  AddCases("query_restore_item_list",152,new Guid("a91b7337-2cad-3eae-8175-e1763f2413ee"),new[]{null,new QueryRestoreItemListMessage()},cases);
  var restoreSlot=new SlotInfo(77L,"longsword_swordwind",1,102,0,null,null,new int[]{1,2,3},180,200,-100,false,"",new Dictionary<string,ItemAttributeElement>{{"ENHANCE",new ItemAttributeElement("ENHANCE","9",0,0)},{"ANTIBIND",new ItemAttributeElement("ANTIBIND","2",0,0)},{"RESTORED",new ItemAttributeElement("RESTORED","1",0,0)}},new Dictionary<int,bool>(),new Dictionary<int,bool>(),0,0,0);
  var restores=new List<RestoreItemInfo>{new RestoreItemInfo(restoreSlot,"AP",300),null,new RestoreItemInfo(null,null,int.MinValue)};
  AddCases("restore_item_list",154,new Guid("b80b34de-ba17-3cef-9ae3-577cfd045708"),new[]{null,new RestoreItemListMessage(null),new RestoreItemListMessage(new List<RestoreItemInfo>()),new RestoreItemListMessage(restores)},cases);
  var enchants=new List<EnchantItemMessage>{null};
  foreach(bool o in new[]{false,true})foreach(long id in new[]{long.MinValue,0L,long.MaxValue})foreach(string d in new[]{null,"","DoEnchant","CancelEnchant","enchant_elixir_lvl1"})enchants.Add(Obj<EnchantItemMessage>("OpenNewSession",o,"TargetItemID",id,"EnchantScrollItemID",id==0L?7L:-id,"IndestructibleScrollItemID",0L,"DiceItemClass",d));
  AddCases("enchant_item",352,new Guid("ddc9fc48-e5dc-304e-bfd2-8c5fed04f83e"),enchants,cases);
  AddCases("enchant_item_result",353,new Guid("0a5b1d79-7e63-32d7-9520-ab7d8bfaa7ed"),new[]{null,new EnchantItemResultMessage(EnchantItemResult.EnchantSuccess,-1,-1,"",0f),new EnchantItemResultMessage(EnchantItemResult.RollSuccess,37,100,"enchant_elixir_lvl1",0.6110000014305115f),new EnchantItemResultMessage(EnchantItemResult.RuneExpired,int.MinValue,int.MaxValue,null,-1f),new EnchantItemResultMessage(EnchantItemResult.ItemDestroyed,0,0,"x",1.5f)},cases);
  var exp=new Dictionary<string,int>{{"heavy_armor",350},{"cooking",int.MinValue},{"",int.MaxValue}};
  var grade=new Dictionary<string,int>{{"heavy_armor",2},{"cooking",0},{"",-1}};
  AddCases("manufacture_info",359,new Guid("9848342d-cc2f-326e-9b4e-d30c6decd5b5"),new[]{null,new ManufactureInfoMessage(null,null,null),new ManufactureInfoMessage(new Dictionary<string,int>(),new Dictionary<string,int>(),new List<string>()),new ManufactureInfoMessage(exp,grade,new List<string>{"heavy_armor_plate_1",null,"","料理"})},cases);
  var crafts=new List<ManufactureCraftMessage>{null};
  foreach(string rid in new[]{null,"","heavy_armor_plate_1"})foreach(var parts in new[]{null,new List<long>(),new List<long>{long.MinValue,0L,long.MaxValue}})crafts.Add(Obj<ManufactureCraftMessage>("RecipeID",rid,"PartsIDList",parts));
  AddCases("manufacture_craft",360,new Guid("42f64ed3-c84b-3cfb-853b-0dec85cfac07"),crafts,cases);
  var levelups=new List<ManufactureLevelUpMessage>{null};
  foreach(string mid in new[]{null,"","heavy_armor"})foreach(int g in new[]{int.MinValue,0,3,int.MaxValue})levelups.Add(Obj<ManufactureLevelUpMessage>("MID",mid,"Grade",g));
  AddCases("manufacture_level_up",361,new Guid("4b8c26ec-6412-3016-b31e-157b18638117"),levelups,cases);
  AddCases("unequippable_parts_info",614,new Guid("d0ac3a73-85f7-38bb-ba07-68bec5d8ae21"),new[]{null,new UnequippablePartsInfoMessage(null),new UnequippablePartsInfoMessage(new List<int>()),new UnequippablePartsInfoMessage(new List<int>{17,18,int.MinValue,int.MaxValue})},cases);
  AddCases("max_durability_repair_item",342,new Guid("66ecfcca-c35b-3005-a79c-ad9fc04da573"),new[]{null,new MaxDurabilityRepairItemMessage(0L,0L),new MaxDurabilityRepairItemMessage(long.MaxValue,long.MinValue),new MaxDurabilityRepairItemMessage(77L,78L)},cases);
  var storages=new List<StorageInfo>{new StorageInfo(0,true,-1,"",0),null,new StorageInfo(2,false,int.MaxValue,"保管",7),new StorageInfo(255,true,int.MinValue,null,int.MinValue)};
  AddCases("update_storage_info",620,new Guid("8eeec067-48d4-3852-9e5e-6a463040bf0c"),new[]{null,new UpdateStorageInfoMessage(null),new UpdateStorageInfoMessage(new List<StorageInfo>()),new UpdateStorageInfoMessage(storages)},cases);
  var starts=new List<SkillEnhanceSessionStartMessage>{null};
  foreach(string sk in new[]{null,"","skill_counter_attack"})foreach(long id in new[]{long.MinValue,0L,long.MaxValue})starts.Add(Obj<SkillEnhanceSessionStartMessage>("SkillName",sk,"SkillEnhanceStoneItemID",id));
  AddCases("skill_enhance_session_start",173,new Guid("c48535b3-8e5e-3483-899d-0fa1c34cf886"),starts,cases);
  var ends=new List<SkillEnhanceSessionEndMessage>{null};
  foreach(string sk in new[]{null,"","skill_counter_attack"})ends.Add(Obj<SkillEnhanceSessionEndMessage>("SkillName",sk));
  AddCases("skill_enhance_session_end",174,new Guid("7b5daeb1-f892-36a4-92f8-f7f5ab24babe"),ends,cases);
  var ergs=new List<SkillEnhanceUseErgMessage>{null};
  foreach(long id in new[]{long.MinValue,0L,long.MaxValue})foreach(int c in new[]{int.MinValue,1,int.MaxValue})ergs.Add(Obj<SkillEnhanceUseErgMessage>("ErgItemID",id,"UseCount",c));
  AddCases("skill_enhance_use_erg",175,new Guid("8a2ad99d-4c45-3e7a-8c27-38c56dc45cda"),ergs,cases);
  AddCases("skill_enhance_use_erg_result",176,new Guid("5a41f49f-bf50-3f92-806c-c16778a0d731"),new[]{null,new SkillEnhanceUseErgResultMessage(true,37,60),new SkillEnhanceUseErgResultMessage(false,int.MinValue,int.MaxValue)},cases);
  var rolls=new List<SkillEnhanceMessage>{null};
  foreach(var ids in new[]{null,new List<long>(),new List<long>{long.MinValue,0L,long.MaxValue}})rolls.Add(Obj<SkillEnhanceMessage>("AdditionalItemIDs",ids));
  AddCases("skill_enhance",177,new Guid("852e561d-1231-3b4e-b53b-d8bec2c34d7e"),rolls,cases);
  var brief=new BriefSkillEnhance{GroupKey=3,IndexKey=-1,Type=2,ReduceDurability=int.MinValue,MaxDurabilityBonus=int.MaxValue};
  AddCases("skill_enhance_result",178,new Guid("8c9bc99c-7e1f-394a-b74e-a12e4b9f4779"),new[]{null,new SkillEnhanceResultMessage(true,"skill_counter_attack",new List<string>(),55,"skillenhance_cooltime_decrease_lv2",false,false,new List<BriefSkillEnhance>{brief}),new SkillEnhanceResultMessage(false,null,new List<string>{"skill_enhance_rune_cash_lv2",null},int.MinValue,null,true,true,new List<BriefSkillEnhance>{null}),new SkillEnhanceResultMessage(false,"",null,0,"",false,true,null)},cases);
  AddCases("skill_enhance_changed",172,new Guid("edfa9396-8cdf-3ccf-811f-7b68c28731a6"),new[]{null,new SkillEnhanceChangedMessage(null),new SkillEnhanceChangedMessage(new Dictionary<string,BriefSkillEnhance>()),new SkillEnhanceChangedMessage(new Dictionary<string,BriefSkillEnhance>{{"skill_counter_attack",brief},{"",new BriefSkillEnhance()},{"skill_x",null}})},cases);
  AddCases("skill_enhance_use_durability",179,new Guid("8d429b16-9c2a-37d5-9fef-1b97a60a1980"),new[]{null,new SkillEnhanceUseDurabilityMessage(0,null),new SkillEnhanceUseDurabilityMessage(-1,new Dictionary<string,int>()),new SkillEnhanceUseDurabilityMessage(int.MaxValue,new Dictionary<string,int>{{"skill_counter_attack",3},{"",int.MinValue}})},cases);
  AddCases("skill_enhance_restore_durability",180,new Guid("05ccc728-0129-3695-b7ca-d5978545b1b3"),new[]{null,new SkillEnhanceRestoreDurabilityMessage(0L,0,null),new SkillEnhanceRestoreDurabilityMessage(long.MaxValue,int.MinValue,"skill_counter_attack"),new SkillEnhanceRestoreDurabilityMessage(long.MinValue,int.MaxValue,"")},cases);
  var colored=new List<ColoredEquipment>{new ColoredEquipment{ItemClass="longsword_saver",Color1=-1,Color2=int.MaxValue,Color3=int.MinValue},null,new ColoredEquipment()};
  AddCases("random_item_reward_list",381,new Guid("4ee2fd65-d464-3707-b876-3280b7d88f04"),new[]{null,new RandomItemRewardListMessage(colored,new List<int>{1,3,int.MinValue},false,""),new RandomItemRewardListMessage(null,null),new RandomItemRewardListMessage(true,"key_x"),new RandomItemRewardListMessage(true,null)},cases);
  AddCases("costume_update",447,new Guid("099b778f-141e-359d-b270-60c9d2f99bbb"),costumes,cases);
  Console.WriteLine(new JavaScriptSerializer{MaxJsonLength=8*1024*1024}.Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",wire_cases=cases}));
 }
}
namespace Utility { public static class Log<T> { public static readonly DummyLogger Logger=new DummyLogger(); } public class DummyLogger {public void DebugFormat(string f,params object[] a){} } }
// EnhanceItemResultMessage's private ItemClassEx helper references the builder; the
// constructor and the serializer never call it.
namespace ServiceCore.ItemServiceOperations { public static class ItemClassExBuilder { public static string Build(string itemClass,int enhanceLevel){throw new NotSupportedException();} } }
namespace Devcat.Core.Memory {public static class ChunkSegment<T>{public static ArraySegment<T> Get(int count){return new ArraySegment<T>(new T[count]);}}}
