use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY, character_list,
    inventory::{self as wire, Kind, Message},
    server_profile::REGISTERED_CLASSES,
};
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn registered(category: i32, type_id: uuid::Uuid) {
    assert!(
        REGISTERED_CLASSES
            .iter()
            .any(|r| r.category_id == category && r.type_id == type_id)
    );
}
/// Unregistered bodies carry the type GUID; registered bodies use the category.
fn fields(category: i32, expected: i32, type_id: uuid::Uuid, body: &[u8]) -> &[u8] {
    if category == expected {
        body
    } else {
        assert_eq!(category, 0);
        // A null object is written with the System.Object GUID and a null marker.
        let null = uuid::Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21);
        if &body[..16] == null.as_bytes() {
            assert_eq!(&body[16..], [0]);
            return &body[16..];
        }
        assert_eq!(&body[..16], type_id.as_bytes());
        &body[16..]
    }
}
fn check_inventory(kind: Kind, case: &Value, n: usize) {
    let category = case["category"].as_i64().unwrap() as i32;
    let body = bytes(&case["body"]);
    let typed = bytes(&case["typed_body"]);
    let null = case["is_null"].as_bool().unwrap();
    let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
    assert_eq!(decoded.is_none(), null, "case {n}");
    assert!(
        wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap() == decoded,
        "case {n}"
    );
    assert_eq!(
        wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY)
            .unwrap()
            .as_slice(),
        typed.as_slice(),
        "case {n}"
    );
    let f = &case["fields"];
    match decoded {
        None => {}
        Some(Message::ItemFail(message)) => {
            let source = &f["Message"];
            assert_eq!(message.is_none(), source.is_null(), "case {n}");
            if let Some(m) = message {
                assert_eq!(m.format.as_deref(), source["Format"].as_str(), "case {n}");
                assert_eq!(
                    m.parameters.map(|p| p.len()),
                    source["Parameters"].as_array().map(Vec::len),
                    "case {n}"
                );
            }
        }
        Some(Message::UpdateInventoryInfo(slots)) => {
            let source = &f["slotInfos"];
            assert_eq!(slots.is_none(), source.is_null(), "case {n}");
            if let Some(slots) = slots {
                let rows = source.as_array().unwrap();
                assert_eq!(slots.len(), rows.len(), "case {n}");
                for (slot, row) in slots.iter().zip(rows) {
                    assert_eq!(
                        slot.as_ref().map(|s| s.id),
                        row["ItemID"].as_i64(),
                        "case {n}"
                    );
                }
            }
        }
        Some(Message::MoveInventoryItem(m)) => {
            assert_eq!(m.item_id, f["ItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                i64::from(m.storage),
                f["Storage"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(i64::from(m.slot), f["Target"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::EquipBundle(b)) => {
            assert_eq!(b.class.as_deref(), f["ItemClass"].as_str(), "case {n}");
            assert_eq!(
                i64::from(b.quick_slot),
                f["QuickSlotID"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::QuickSlotInfo(info)) => {
            assert_eq!(info.is_none(), f["info"].is_null(), "case {n}");
        }
        Some(
            Message::QueryAp
            | Message::QueryCashShopProductList
            | Message::QueryCashShopBalance
            | Message::QueryCashShopInventory,
        ) => {}
        Some(Message::Ap(a)) => {
            assert_eq!(
                a.next_bonus_ms,
                f["NextBonusTimeTicks"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(i64::from(a.ap), f["AP"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                i64::from(a.max_ap),
                f["MaxAP"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(a.bonus_interval),
                f["APBonusInterval"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::UseInventoryItem(u)) => {
            assert_eq!(u.item_id, f["ItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                u.target_item_id,
                f["TargetItemID"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::UseInventoryItemOk(class)) => {
            assert_eq!(class.as_deref(), f["UseItemClass"].as_str(), "case {n}");
        }
        Some(Message::QuerySkillList) => {}
        Some(Message::SkillList(l)) => {
            assert_eq!(
                l.learning.as_deref(),
                f["LearningSkillID"].as_str(),
                "case {n}"
            );
            assert_eq!(
                i64::from(l.current_ap),
                f["CurrentAp"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                l.reset_vocation,
                f["IsResetVocation"].as_bool().unwrap(),
                "case {n}"
            );
            let rows = f["SkillList"].as_array();
            assert_eq!(
                l.skills.as_ref().map(Vec::len),
                rows.map(Vec::len),
                "case {n}"
            );
            if let (Some(skills), Some(rows)) = (l.skills, rows) {
                for (skill, row) in skills.iter().zip(rows) {
                    assert_eq!(skill.is_none(), row.is_null(), "case {n}");
                    if let Some(b) = skill {
                        assert_eq!(b.id.as_deref(), row["SkillID"].as_str(), "case {n}");
                        assert_eq!(
                            i64::from(b.untrain_ap),
                            row["UntrainSkillAP"].as_i64().unwrap(),
                            "case {n}"
                        );
                        assert_eq!(
                            b.enhances.as_ref().map(Vec::len),
                            row["Enhances"].as_array().map(Vec::len),
                            "case {n}"
                        );
                    }
                }
            }
        }
        Some(Message::LearnSkill(v)) => {
            assert_eq!(v.skill.as_deref(), f["SkillID"].as_str(), "case {n}");
            assert_eq!(i64::from(v.value), f["AP"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::SkillCompletion(v)) => {
            assert_eq!(v.skill.as_deref(), f["SkillID"].as_str(), "case {n}");
            assert_eq!(
                i64::from(v.value),
                f["SkillRank"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::QueryCharacterCommonInfo(q)) => {
            assert_eq!(q.query_id, f["QueryID"].as_i64().unwrap(), "case {n}");
            assert_eq!(q.cid, f["CID"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::LearnNewSkill(skill)) => {
            assert_eq!(skill.as_deref(), f["SkillID"].as_str(), "case {n}");
        }
        Some(Message::LearnNewSkillResult(result)) => {
            assert_eq!(i64::from(result), f["result"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::QueryNpcList) | Some(Message::ExecuteNpcServerCommand) => {}
        Some(Message::NpcList(buildings)) => {
            let expected = f["Buildings"].as_array();
            assert_eq!(
                buildings.as_ref().map(Vec::len),
                expected.map(Vec::len),
                "case {n}"
            );
            for (b, e) in buildings
                .iter()
                .flatten()
                .zip(expected.into_iter().flatten())
            {
                assert_eq!(b.is_none(), e.is_null(), "case {n}");
                let Some(b) = b else { continue };
                assert_eq!(b.building.as_deref(), e["BuildingID"].as_str(), "case {n}");
                let npcs = e["Npcs"].as_array();
                assert_eq!(
                    b.npcs.as_ref().map(Vec::len),
                    npcs.map(Vec::len),
                    "case {n}"
                );
                for (npc, e) in b.npcs.iter().flatten().zip(npcs.into_iter().flatten()) {
                    assert_eq!(npc.is_none(), e.is_null(), "case {n}");
                    let Some(npc) = npc else { continue };
                    assert_eq!(npc.npc.as_deref(), e["NpcID"].as_str(), "case {n}");
                    assert_eq!(npc.feature.as_deref(), e["Feature"].as_str(), "case {n}");
                    let lines = e["StoryLines"].as_array();
                    assert_eq!(
                        npc.story_lines.as_ref().map(Vec::len),
                        lines.map(Vec::len),
                        "case {n}"
                    );
                    for (l, e) in npc
                        .story_lines
                        .iter()
                        .flatten()
                        .zip(lines.into_iter().flatten())
                    {
                        assert_eq!(l.is_none(), e.is_null(), "case {n}");
                        let Some(l) = l else { continue };
                        assert_eq!(
                            l.story_line.as_deref(),
                            e["StoryLineID"].as_str(),
                            "case {n}"
                        );
                        assert_eq!(Some(l.is_revealing), e["IsRevealing"].as_bool(), "case {n}");
                    }
                }
            }
        }
        Some(Message::QueryNpcTalk(q)) => {
            assert_eq!(q.location.as_deref(), f["Location"].as_str(), "case {n}");
            assert_eq!(q.npc.as_deref(), f["NpcID"].as_str(), "case {n}");
            assert_eq!(q.story_line.as_deref(), f["StoryLine"].as_str(), "case {n}");
            assert_eq!(q.command.as_deref(), f["Command"].as_str(), "case {n}");
        }
        Some(Message::NpcTalk(content)) => {
            let expected = f["Content"].as_array();
            assert_eq!(
                content.as_ref().map(Vec::len),
                expected.map(Vec::len),
                "case {n}"
            );
            for (t, e) in content.iter().flatten().zip(expected.into_iter().flatten()) {
                assert_eq!(t.is_none(), e.is_null(), "case {n}");
                let Some(t) = t else { continue };
                assert_eq!(t.command.as_deref(), e["Command"].as_str(), "case {n}");
                assert_eq!(t.arg1.as_deref(), e["Arg1"].as_str(), "case {n}");
                assert_eq!(t.arg2.as_deref(), e["Arg2"].as_str(), "case {n}");
            }
        }
        Some(Message::StoryLines(lines)) => {
            assert_eq!(
                i64::from(lines.hint_category),
                f["HintCategory"].as_i64().unwrap(),
                "case {n}"
            );
            let expected = f["StoryStatus"].as_array();
            assert_eq!(
                lines.status.as_ref().map(Vec::len),
                expected.map(Vec::len),
                "case {n}"
            );
            for (b, e) in lines
                .status
                .iter()
                .flatten()
                .zip(expected.into_iter().flatten())
            {
                assert_eq!(b.is_none(), e.is_null(), "case {n}");
                let Some(b) = b else { continue };
                assert_eq!(b.story_line.as_deref(), e["StoryLine"].as_str(), "case {n}");
                assert_eq!(b.phase.as_deref(), e["Phase"].as_str(), "case {n}");
                assert_eq!(
                    i64::from(b.status),
                    e["Status"].as_i64().unwrap(),
                    "case {n}"
                );
                assert_eq!(b.phase_text.as_deref(), e["PhaseText"].as_str(), "case {n}");
            }
        }
        Some(Message::BuyItem(b) | Message::RequestCraft(b)) => {
            assert_eq!(b.shop_id.as_deref(), f["ShopID"].as_str(), "case {n}");
            assert_eq!(i64::from(b.order), f["Order"].as_i64().unwrap(), "case {n}");
            assert_eq!(i64::from(b.count), f["Count"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::BuyItemResult(b)) => {
            assert_eq!(
                i64::from(b.result),
                f["Result"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(b.item_class.as_deref(), f["ItemClass"].as_str(), "case {n}");
            assert_eq!(
                i64::from(b.item_count),
                f["ItemCount"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                b.price_item_class.as_deref(),
                f["PriceItemClass"].as_str(),
                "case {n}"
            );
            assert_eq!(
                i64::from(b.price_item_count),
                f["PriceItemCount"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(b.restriction_order),
                f["RestrictionItemOrder"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(b.restriction_count),
                f["RestrictionItemCount"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::QueryShopItemInfo(shop)) => {
            assert_eq!(shop.as_deref(), f["ShopID"].as_str(), "case {n}");
        }
        Some(Message::SellItem(sell)) => {
            assert_eq!(sell.item_id, f["ItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                i64::from(sell.count),
                f["Count"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::QueryBuybackList) => {}
        Some(Message::BuybackListResult(list)) => {
            let expected = f["BuybackList"].as_array();
            assert_eq!(
                list.as_ref().map(Vec::len),
                expected.map(Vec::len),
                "case {n}"
            );
            for (b, e) in list.iter().flatten().zip(expected.into_iter().flatten()) {
                assert_eq!(b.is_none(), e.is_null(), "case {n}");
                let Some(b) = b else { continue };
                assert_eq!(b.shop_id.as_deref(), e["ShopID"].as_str(), "case {n}");
                assert_eq!(i64::from(b.order), e["Order"].as_i64().unwrap(), "case {n}");
                assert_eq!(b.item_class.as_deref(), e["ItemClass"].as_str(), "case {n}");
                assert_eq!(
                    i64::from(b.item_count),
                    e["ItemCount"].as_i64().unwrap(),
                    "case {n}"
                );
                assert_eq!(
                    b.price_item_class.as_deref(),
                    e["PriceItemClass"].as_str(),
                    "case {n}"
                );
                assert_eq!(
                    i64::from(b.price_count),
                    e["PriceCount"].as_i64().unwrap(),
                    "case {n}"
                );
            }
        }
        Some(Message::TitleList(list)) => {
            for (got, key) in [(&list.account, "AccountTitles"), (&list.titles, "Titles")] {
                let expected = f[key].as_array();
                assert_eq!(
                    got.as_ref().map(Vec::len),
                    expected.map(Vec::len),
                    "case {n}"
                );
                for (v, e) in got.iter().flatten().zip(expected.into_iter().flatten()) {
                    assert_eq!(v.is_none(), e.is_null(), "case {n}");
                    if let Some(v) = v {
                        assert_eq!(
                            i64::from(v.title_id),
                            e["TitleID"].as_i64().unwrap(),
                            "case {n}"
                        );
                        assert_eq!(
                            i64::from(v.status),
                            e["Status"]["value__"].as_i64().unwrap(),
                            "case {n}"
                        );
                    }
                }
            }
        }
        Some(Message::QueryRestoreItemList) => {}
        Some(Message::EnchantItem(e)) => {
            assert_eq!(
                Some(e.open_new_session),
                f["OpenNewSession"].as_bool(),
                "case {n}"
            );
            assert_eq!(
                e.target_item_id,
                f["TargetItemID"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                e.enchant_scroll_item_id,
                f["EnchantScrollItemID"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                e.dice_item_class.as_deref(),
                f["DiceItemClass"].as_str(),
                "case {n}"
            );
        }
        Some(Message::EnchantItemResult(e)) => {
            assert_eq!(
                i64::from(e.result),
                f["Result"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(e.current_success_ratio),
                f["CurrentSuccessRatio"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                e.rolled_dice.as_deref(),
                f["RolledDice"].as_str(),
                "case {n}"
            );
        }
        Some(Message::RestoreItemList(list)) => {
            let source = &f["RestoreItemList"];
            assert_eq!(list.is_none(), source.is_null(), "case {n}");
            if let Some(list) = list {
                let rows = source.as_array().unwrap();
                assert_eq!(list.len(), rows.len(), "case {n}");
                for (info, row) in list.iter().zip(rows) {
                    assert_eq!(info.is_none(), row.is_null(), "case {n}");
                    if let Some(info) = info {
                        assert_eq!(
                            info.price_type.as_deref(),
                            row["PriceType"].as_str(),
                            "case {n}"
                        );
                        assert_eq!(
                            i64::from(info.price_value),
                            row["PriceValue"].as_i64().unwrap(),
                            "case {n}"
                        );
                        assert_eq!(
                            info.slot.as_ref().map(|s| (
                                s.id,
                                s.tab,
                                s.attributes.as_ref().map(Vec::len)
                            )),
                            row["Slot"].as_object().map(|s| (
                                s["ItemID"].as_i64().unwrap(),
                                i32::try_from(s["Tab"].as_i64().unwrap()).unwrap(),
                                s["Attributes"].as_array().map(Vec::len)
                            )),
                            "case {n}"
                        );
                    }
                }
            }
        }
        Some(Message::EnhanceItem(e)) => {
            assert_eq!(e.item_id, f["ItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(e.material1.as_deref(), f["Material1"].as_str(), "case {n}");
            assert_eq!(e.material2.as_deref(), f["Material2"].as_str(), "case {n}");
            assert_eq!(
                e.additional_material.as_deref(),
                f["AdditionalMaterial"].as_str(),
                "case {n}"
            );
            assert_eq!(
                Some(e.is_event_enhance_a_shot),
                f["IsEventEnhanceAShot"].as_bool(),
                "case {n}"
            );
        }
        Some(Message::EnhanceItemResult(e)) => {
            assert_eq!(e.result.as_deref(), f["Result"].as_str(), "case {n}");
            assert_eq!(e.item_id, f["ItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                e.prev_item_class.as_deref(),
                f["PrevItemClass"].as_str(),
                "case {n}"
            );
            assert_eq!(
                e.enhanced_item_class.as_deref(),
                f["EnhancedItemClass"].as_str(),
                "case {n}"
            );
            let rewards = f["FailReward"].as_array().map(|o| {
                o.iter()
                    .map(|p| {
                        (
                            p["key"].as_str().unwrap().to_owned(),
                            i32::try_from(p["value"].as_i64().unwrap()).unwrap(),
                        )
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(e.fail_reward.0, rewards, "case {n}");
        }
        Some(Message::ManufactureInfo(m)) => {
            let dict = |v: &Value| {
                v.as_array().map(|o| {
                    o.iter()
                        .map(|p| {
                            (
                                p["key"].as_str().unwrap().to_owned(),
                                i32::try_from(p["value"].as_i64().unwrap()).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            };
            assert_eq!(m.exp.0, dict(&f["ExpDictionary"]), "case {n}");
            assert_eq!(m.grade.0, dict(&f["GradeDictionary"]), "case {n}");
            let recipes = f["Recipes"]
                .as_array()
                .map(|o| o.iter().map(|r| r.as_str().map(str::to_owned)).collect());
            assert_eq!(m.recipes, recipes, "case {n}");
        }
        Some(Message::ManufactureCraft(c)) => {
            assert_eq!(c.recipe_id.as_deref(), f["RecipeID"].as_str(), "case {n}");
            let parts = f["PartsIDList"]
                .as_array()
                .map(|o| o.iter().map(|v| v.as_i64().unwrap()).collect());
            assert_eq!(c.parts_id_list, parts, "case {n}");
        }
        Some(Message::SkillEnhanceSessionStart(m)) => {
            assert_eq!(m.skill_name.as_deref(), f["SkillName"].as_str(), "case {n}");
            assert_eq!(
                m.stone_item_id,
                f["SkillEnhanceStoneItemID"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::SkillEnhanceSessionEnd(name)) => {
            assert_eq!(name.as_deref(), f["SkillName"].as_str(), "case {n}");
        }
        Some(Message::SkillEnhanceUseErg(m)) => {
            assert_eq!(m.erg_item_id, f["ErgItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                i64::from(m.use_count),
                f["UseCount"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::SkillEnhanceUseErgResult(m)) => {
            assert_eq!(Some(m.result), f["Result"].as_bool(), "case {n}");
            assert_eq!(
                i64::from(m.current),
                f["CurrentPercentage"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(m.max),
                f["MaxPercentage"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::SkillEnhance(ids)) => {
            let source = f["AdditionalItemIDs"]
                .as_array()
                .map(|o| o.iter().map(|v| v.as_i64().unwrap()).collect());
            assert_eq!(ids, source, "case {n}");
        }
        Some(Message::SkillEnhanceResult(m)) => {
            assert_eq!(m.skill_name.as_deref(), f["SkillName"].as_str(), "case {n}");
            assert_eq!(
                m.stone_item.as_deref(),
                f["SkillEnhanceStoneItem"].as_str(),
                "case {n}"
            );
            assert_eq!(
                i64::from(m.success_ratio),
                f["SuccessRatio"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(Some(m.result), f["Result"].as_bool(), "case {n}");
            assert_eq!(
                (Some(m.additional_item_destroyed), Some(m.stone_protected)),
                (
                    f["IsAdditionalItemDestroyed"].as_bool(),
                    f["IsEnhanceStoneProtected"].as_bool()
                ),
                "case {n}"
            );
            let classes = f["AdditionalItemClasses"]
                .as_array()
                .map(|o| o.iter().map(|v| v.as_str().map(str::to_owned)).collect());
            assert_eq!(m.additional_item_classes, classes, "case {n}");
            assert_eq!(
                m.enhances.as_ref().map(Vec::len),
                f["Enhances"].as_array().map(Vec::len),
                "case {n}"
            );
        }
        Some(Message::SkillEnhanceChanged(m)) => {
            let source = f["SkillEnhanceChanged"].as_array().map(|o| {
                o.iter()
                    .map(|p| {
                        (
                            p["key"].as_str().unwrap().to_owned(),
                            (!p["value"].is_null())
                                .then(|| p["value"]["GroupKey"].as_i64().unwrap()),
                        )
                    })
                    .collect::<Vec<_>>()
            });
            let ours = m.0.as_ref().map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.as_ref().map(|e| i64::from(e.group_key))))
                    .collect::<Vec<_>>()
            });
            assert_eq!(ours, source, "case {n}");
        }
        Some(Message::SkillEnhanceUseDurability(m)) => {
            assert_eq!(i64::from(m.slot), f["Slot"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                m.use_durability.0.as_ref().map(Vec::len),
                f["UseDurability"].as_array().map(Vec::len),
                "case {n}"
            );
        }
        Some(Message::SkillEnhanceRestoreDurability(m)) => {
            assert_eq!(
                m.stone_item_id,
                f["EnhanceStoneItemID"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(m.use_count),
                f["UseCount"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(m.skill_name.as_deref(), f["SkillName"].as_str(), "case {n}");
        }
        Some(Message::CashShopCategoryList(list)) => {
            let ours = list.as_ref().map(|l| {
                l.iter()
                    .map(|c| c.as_ref().map(|c| (c.category_no, c.category_name.clone())))
                    .collect::<Vec<_>>()
            });
            let theirs = f["list"].as_array().map(|l| {
                l.iter()
                    .map(|c| {
                        (!c.is_null()).then(|| {
                            (
                                i32::try_from(c["CategoryNo"].as_i64().unwrap()).unwrap(),
                                c["CategoryName"].as_str().map(str::to_owned),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(ours, theirs, "case {n}");
        }
        Some(Message::CashShopProductList(list)) => {
            let ours = list.as_ref().map(|l| {
                l.iter()
                    .map(|p| {
                        p.as_ref()
                            .map(|p| (p.product_no, p.product_id.clone(), p.sale_price, p.status))
                    })
                    .collect::<Vec<_>>()
            });
            let theirs = f["list"].as_array().map(|l| {
                l.iter()
                    .map(|p| {
                        (!p.is_null()).then(|| {
                            (
                                i32::try_from(p["ProductNo"].as_i64().unwrap()).unwrap(),
                                p["ProductID"].as_str().map(str::to_owned),
                                i32::try_from(p["SalePrice"].as_i64().unwrap()).unwrap(),
                                i32::try_from(p["Status"].as_i64().unwrap()).unwrap(),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(ours, theirs, "case {n}");
        }
        Some(Message::CashShopBalance(b)) => {
            assert_eq!(
                (i64::from(b.balance), i64::from(b.refundless)),
                (
                    f["Balance"].as_i64().unwrap(),
                    f["Refundless"].as_i64().unwrap()
                ),
                "case {n}"
            );
        }
        Some(Message::CashShopInventory(list)) => {
            let ours = list.as_ref().map(|l| {
                l.iter()
                    .map(|o| {
                        o.as_ref()
                            .map(|o| (o.order_no, o.item_class_ex.clone(), o.count, o.is_gift))
                    })
                    .collect::<Vec<_>>()
            });
            let theirs = f["Inventory"].as_array().map(|l| {
                l.iter()
                    .map(|o| {
                        (!o.is_null()).then(|| {
                            (
                                i32::try_from(o["OrderNo"].as_i64().unwrap()).unwrap(),
                                o["ItemClassEx"].as_str().map(str::to_owned),
                                i16::try_from(o["Count"].as_i64().unwrap()).unwrap(),
                                o["IsGift"].as_bool().unwrap(),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(ours, theirs, "case {n}");
        }
        Some(Message::QueryCashShopItemPickUp(p)) => {
            assert_eq!(
                (
                    i64::from(p.order_no),
                    i64::from(p.product_no),
                    i64::from(p.quantity)
                ),
                (
                    f["OrderNo"].as_i64().unwrap(),
                    f["ProductNo"].as_i64().unwrap(),
                    f["Quantity"].as_i64().unwrap()
                ),
                "case {n}"
            );
        }
        Some(Message::QueryCashShopPurchaseItem(p)) => {
            assert_eq!(
                (
                    i64::from(p.product_no),
                    i64::from(p.quantity),
                    p.attribute3.as_deref(),
                    p.is_for_common_inven
                ),
                (
                    f["ProductNo"].as_i64().unwrap(),
                    f["Quantity"].as_i64().unwrap(),
                    f["Attribute3"].as_str(),
                    f["IsForCommonInven"].as_bool().unwrap()
                ),
                "case {n}"
            );
        }
        Some(Message::CashShopFail(r)) => {
            assert_eq!(i64::from(r), f["Reason"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::WishListSelect(cid)) => {
            assert_eq!(cid, f["CID"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::WishListSelectResponse(r)) => {
            assert_eq!(
                (i64::from(r.result), r.cid),
                (
                    f["Result"]["value__"].as_i64().unwrap(),
                    f["CID"].as_i64().unwrap(),
                ),
                "case {n}"
            );
            let ours = r.product_info.as_ref().map(|l| {
                l.iter()
                    .map(|w| w.as_ref().map(|w| (w.product_no, w.product_name.clone())))
                    .collect::<Vec<_>>()
            });
            let theirs = f["ProductInfo"].as_array().map(|l| {
                l.iter()
                    .map(|w| {
                        (!w.is_null()).then(|| {
                            (
                                i32::try_from(w["ProductNo"].as_i64().unwrap()).unwrap(),
                                w["ProductName"].as_str().map(str::to_owned),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(ours, theirs, "case {n}");
        }
        Some(Message::WishListInsert(c) | Message::WishListDelete(c)) => {
            assert_eq!(c.cid, f["CID"].as_i64().unwrap(), "case {n}");
            let list = if f["List_Wish"].is_null() && !f["ProductNo"].is_null() {
                &f["ProductNo"]
            } else {
                &f["List_Wish"]
            };
            let theirs = list.as_array().map(|l| {
                l.iter()
                    .map(|x| i32::try_from(x.as_i64().unwrap()).unwrap())
                    .collect::<Vec<_>>()
            });
            assert_eq!(c.products, theirs, "case {n}");
        }
        Some(Message::WishListInsertResponse(r) | Message::WishListDeleteResponse(r)) => {
            assert_eq!(
                (i64::from(r.result), r.cid),
                (
                    f["Result"]["value__"].as_i64().unwrap(),
                    f["CID"].as_i64().unwrap(),
                ),
                "case {n}"
            );
        }
        Some(Message::BeautyShopCustomize(b)) => {
            let ours = b.customize_items.as_ref().map(|l| {
                l.iter()
                    .map(|c| {
                        c.as_ref()
                            .map(|c| (c.item_class.clone(), c.duration, c.price.clone()))
                    })
                    .collect::<Vec<_>>()
            });
            let theirs = f["CustomizeItems"].as_array().map(|l| {
                l.iter()
                    .map(|c| {
                        (!c.is_null()).then(|| {
                            (
                                c["ItemClass"].as_str().map(str::to_owned),
                                i32::try_from(c["Duration"].as_i64().unwrap()).unwrap(),
                                c["Price"].as_str().map(str::to_owned),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(ours, theirs, "case {n}");
            assert_eq!(
                (i64::from(b.payment), b.eyebrow_item_class.as_deref()),
                (
                    f["payment"].as_i64().unwrap(),
                    f["EyebrowItemClass"].as_str()
                ),
                "case {n}"
            );
            let shapes = f["BodyShapeInfo"].as_array().map(|l| {
                l.iter()
                    .map(|e| {
                        (
                            i32::try_from(e["key"].as_i64().unwrap()).unwrap(),
                            (e["value"].as_f64().unwrap() as f32).to_bits() as i32,
                        )
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(b.body_shape_info.0, shapes, "case {n}");
        }
        Some(Message::RequestSecuredOperation(op)) => {
            assert_eq!(
                i64::from(op),
                f["Operation"]["value__"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::RequestSortInventory(no)) => {
            assert_eq!(i64::from(no), f["StorageNo"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::StatusEffectUpdated(m)) => {
            assert_eq!(
                m.character_name.as_deref(),
                f["CharacterName"].as_str(),
                "case {n}"
            );
            let ours: Option<Vec<Option<(Option<String>, i64, i64, i64)>>> =
                m.status_effects.as_ref().map(|l| {
                    l.iter()
                        .map(|e| {
                            e.as_ref().map(|e| {
                                (
                                    e.kind.clone(),
                                    i64::from(e.level),
                                    i64::from(e.remain_time),
                                    i64::from(e.combat_count),
                                )
                            })
                        })
                        .collect()
                });
            let theirs = f["StatusEffects"].as_array().map(|l| {
                l.iter()
                    .map(|e| {
                        (!e.is_null()).then(|| {
                            (
                                e["Type"].as_str().map(str::to_owned),
                                e["Level"].as_i64().unwrap(),
                                e["RemainTime"].as_i64().unwrap(),
                                e["CombatCount"].as_i64().unwrap(),
                            )
                        })
                    })
                    .collect()
            });
            assert_eq!(ours, theirs, "case {n}");
        }
        Some(Message::UseExtraStorage(op)) => {
            assert_eq!(
                (op.item_id, i64::from(op.storage_id)),
                (
                    f["ItemID"].as_i64().unwrap(),
                    f["StorageID"].as_i64().unwrap()
                ),
                "case {n}"
            );
        }
        Some(Message::SecuredOperation(op)) => {
            assert_eq!(
                (
                    i64::from(op.operation),
                    i64::from(op.locked_time_in_seconds)
                ),
                (
                    f["Operation"]["value__"].as_i64().unwrap(),
                    f["LockedTimeInSeconds"].as_i64().unwrap()
                ),
                "case {n}"
            );
        }
        Some(Message::RandomItemRewardList(m)) => {
            let classes = f["ItemClasses"].as_array().map(|o| {
                o.iter()
                    .map(|v| (!v.is_null()).then(|| v["ItemClass"].as_str().map(str::to_owned)))
                    .collect::<Vec<_>>()
            });
            let ours = m.item_classes.as_ref().map(|o| {
                o.iter()
                    .map(|c| c.as_ref().map(|c| c.item_class.clone()))
                    .collect::<Vec<_>>()
            });
            assert_eq!(ours, classes, "case {n}");
            let quantities = f["ItemQuantities"].as_array().map(|o| {
                o.iter()
                    .map(|v| i32::try_from(v.as_i64().unwrap()).unwrap())
                    .collect()
            });
            assert_eq!(m.item_quantities, quantities, "case {n}");
            assert_eq!(
                (
                    Some(m.is_user_care),
                    m.key_item_class.as_deref(),
                    Some(m.disable_show_pop_up)
                ),
                (
                    f["IsUserCare"].as_bool(),
                    f["KeyItemClass"].as_str(),
                    f["DisableShowPopUp"].as_bool()
                ),
                "case {n}"
            );
        }
        Some(Message::UpdateStorageInfo(list)) => {
            let source = &f["StorageInfos"];
            assert_eq!(list.is_none(), source.is_null(), "case {n}");
            if let Some(list) = list {
                let rows = source.as_array().unwrap();
                assert_eq!(list.len(), rows.len(), "case {n}");
                for (info, row) in list.iter().zip(rows) {
                    assert_eq!(info.is_none(), row.is_null(), "case {n}");
                    if let Some(info) = info {
                        assert_eq!(
                            (
                                i64::from(info.id),
                                i64::from(info.available),
                                i64::from(info.last_seconds),
                                info.name.as_deref(),
                                i64::from(info.tag)
                            ),
                            (
                                row["StorageID"].as_i64().unwrap(),
                                row["IsAvailable"].as_i64().unwrap(),
                                row["LastSecsToExpire"].as_i64().unwrap(),
                                row["StorageName"].as_str(),
                                row["StorageTag"].as_i64().unwrap()
                            ),
                            "case {n}"
                        );
                    }
                }
            }
        }
        Some(Message::MaxDurabilityRepairItem(m)) => {
            assert_eq!(
                m.target_item_id,
                f["TargetItemID"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                m.source_item_id,
                f["SourceItemID"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::UnequippablePartsInfo(parts)) => {
            let source = f["Parts"].as_array().map(|o| {
                o.iter()
                    .map(|v| i32::try_from(v.as_i64().unwrap()).unwrap())
                    .collect()
            });
            assert_eq!(parts, source, "case {n}");
        }
        Some(Message::ManufactureLevelUp(l)) => {
            assert_eq!(l.mid.as_deref(), f["MID"].as_str(), "case {n}");
            assert_eq!(i64::from(l.grade), f["Grade"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::HideCostume(h)) => {
            assert_eq!(Some(h.hide_head), f["HideHead"].as_bool(), "case {n}");
            assert_eq!(
                i64::from(h.avatar_part),
                f["AvatarPart"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(h.avatar_state),
                f["AvatarState"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::RepairItem(r)) => {
            let ids = f["ItemIDs"]
                .as_array()
                .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect::<Vec<_>>());
            assert_eq!(r.item_ids, ids, "case {n}");
            assert_eq!(
                Some(r.all_equipped),
                f["AddAllEquippedItems"].as_bool(),
                "case {n}"
            );
            assert_eq!(
                Some(r.all_broken),
                f["AddAllBrokenItems"].as_bool(),
                "case {n}"
            );
            assert_eq!(
                Some(r.all_repairable),
                f["AddAllRepairableItems"].as_bool(),
                "case {n}"
            );
        }
        Some(Message::UpdateStoryGuide(g)) => {
            assert_eq!(
                g.target.as_deref(),
                f["TargetLandMark"].as_str(),
                "case {n}"
            );
            assert_eq!(g.message.as_deref(), f["GuideMessage"].as_str(), "case {n}");
        }
        Some(Message::QueryQuestProgress) => {}
        Some(Message::UpdateTitle(titles)) => {
            let expected = f["Titles"].as_array();
            assert_eq!(
                titles.as_ref().map(Vec::len),
                expected.map(Vec::len),
                "case {n}"
            );
            for (v, e) in titles.iter().flatten().zip(expected.into_iter().flatten()) {
                assert_eq!(v.is_none(), e.is_null(), "case {n}");
                let Some(v) = v else { continue };
                assert_eq!(
                    i64::from(v.title_id),
                    e["TitleID"].as_i64().unwrap(),
                    "case {n}"
                );
                assert_eq!(
                    v.expire_date_time,
                    e["ExpireDateTime"].as_i64().unwrap(),
                    "case {n}"
                );
            }
        }
        Some(Message::QuestProgress(q)) => {
            let expected = f["QuestProgress"].as_array();
            assert_eq!(
                q.progress.as_ref().map(Vec::len),
                expected.map(Vec::len),
                "case {n}"
            );
            for (v, e) in q
                .progress
                .iter()
                .flatten()
                .zip(expected.into_iter().flatten())
            {
                assert_eq!(v.is_none(), e.is_null(), "case {n}");
                let Some(v) = v else { continue };
                assert_eq!(v.id.as_deref(), e["ID"].as_str(), "case {n}");
                assert_eq!(
                    i64::from(v.max_play_count),
                    e["MaxPlayCount"].as_i64().unwrap(),
                    "case {n}"
                );
                assert_eq!(
                    v.today_quest,
                    e["TodayQuest"].as_bool().unwrap(),
                    "case {n}"
                );
                assert_eq!(
                    v.clear_times.as_ref().map(Vec::len),
                    e["ClearTimes"].as_array().map(Vec::len),
                    "case {n}"
                );
            }
            let goals = f["AchievedGoals"].as_array();
            assert_eq!(
                q.achieved_goals.as_ref().map(Vec::len),
                goals.map(Vec::len),
                "case {n}"
            );
            for (v, e) in q
                .achieved_goals
                .iter()
                .flatten()
                .zip(goals.into_iter().flatten())
            {
                let Some(v) = v else { continue };
                assert_eq!(
                    i64::from(v.regened_ap),
                    e["RegenedAP"].as_i64().unwrap(),
                    "case {n}"
                );
                assert_eq!(
                    i64::from(v.last_clear_time_diff),
                    e["LastClearTimeDiff"].as_i64().unwrap(),
                    "case {n}"
                );
            }
        }
        Some(Message::StoryVariables(vars)) => {
            let expected = f["StoryVariables"].as_array();
            assert_eq!(
                vars.as_ref().map(Vec::len),
                expected.map(Vec::len),
                "case {n}"
            );
            for (v, e) in vars.iter().flatten().zip(expected.into_iter().flatten()) {
                assert_eq!(v.is_none(), e.is_null(), "case {n}");
                let Some(v) = v else { continue };
                assert_eq!(v.story_line.as_deref(), e["StoryLine"].as_str(), "case {n}");
                assert_eq!(v.key.as_deref(), e["Key"].as_str(), "case {n}");
                assert_eq!(i64::from(v.value), e["Value"].as_i64().unwrap(), "case {n}");
            }
        }
        Some(Message::SelectButton(index)) => {
            assert_eq!(
                i64::from(index),
                f["ButtonIndex"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::SetStoryLineStatus(v)) => {
            assert_eq!(
                v.story_line.as_deref(),
                f["StoryLineID"].as_str(),
                "case {n}"
            );
            assert_eq!(
                i64::from(v.status),
                f["Status"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(_) => panic!("unexpected kind"),
    }
    for cut in 0..typed.len() {
        assert!(wire::decode(kind, &typed[..cut], DEFAULT_MAX_BODY).is_err());
    }
    let mut extra = typed.clone();
    extra.push(0);
    assert!(wire::decode(kind, &extra, DEFAULT_MAX_BODY).is_err());
}
fn check_costume(case: &Value, n: usize) {
    let category = case["category"].as_i64().unwrap() as i32;
    let body = bytes(&case["body"]);
    let typed = bytes(&case["typed_body"]);
    let null = case["is_null"].as_bool().unwrap();
    let decoded = character_list::decode_costume_update(&typed, DEFAULT_MAX_BODY).unwrap();
    assert_eq!(decoded.is_none(), null, "case {n}");
    let ingress = fields(
        category,
        character_list::COSTUME_UPDATE_CATEGORY,
        character_list::COSTUME_UPDATE_TYPE_ID,
        &body,
    );
    assert!(
        character_list::decode_costume_update(ingress, DEFAULT_MAX_BODY).unwrap() == decoded,
        "case {n}"
    );
    assert_eq!(
        character_list::encode_costume_update(
            decoded.as_ref().map(Option::as_ref),
            DEFAULT_MAX_BODY
        )
        .unwrap()
        .as_slice(),
        typed.as_slice(),
        "case {n}"
    );
    if let Some(info) = decoded {
        let source = &case["fields"]["CostumeInfo"];
        assert_eq!(info.is_none(), source.is_null(), "case {n}");
        if let Some(c) = info {
            assert_eq!(
                i64::from(c.shineness),
                source["Shineness"].as_i64().unwrap()
            );
            assert_eq!(
                i64::from(c.town_effect),
                source["TownEffect"].as_i64().unwrap()
            );
            assert_eq!(
                c.costume_type_info.map(|d| d.len()),
                source["CostumeTypeInfo"].as_array().map(Vec::len)
            );
        }
    }
    for cut in 0..typed.len() {
        assert!(character_list::decode_costume_update(&typed[..cut], DEFAULT_MAX_BODY).is_err());
    }
}
#[test]
fn framework_item_sync_envelopes_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-item-sync-wire.json")).unwrap();
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    registered(
        Kind::UpdateInventoryInfo.category(),
        Kind::UpdateInventoryInfo.type_id(),
    );
    registered(Kind::ItemFail.category(), Kind::ItemFail.type_id());
    registered(Kind::EquipBundle.category(), Kind::EquipBundle.type_id());
    registered(
        Kind::QuickSlotInfo.category(),
        Kind::QuickSlotInfo.type_id(),
    );
    registered(
        Kind::MoveInventoryItem.category(),
        Kind::MoveInventoryItem.type_id(),
    );
    registered(
        character_list::COSTUME_UPDATE_CATEGORY,
        character_list::COSTUME_UPDATE_TYPE_ID,
    );
    for kind in [
        Kind::QueryAp,
        Kind::Ap,
        Kind::UseInventoryItem,
        Kind::UseInventoryItemOk,
        Kind::QuerySkillList,
        Kind::SkillList,
        Kind::LearnSkill,
        Kind::SkillCompletion,
        Kind::QueryCharacterCommonInfo,
        Kind::LearnNewSkill,
        Kind::LearnNewSkillResult,
        Kind::QueryNpcList,
        Kind::NpcList,
        Kind::QueryNpcTalk,
        Kind::NpcTalk,
        Kind::SelectButton,
        Kind::ExecuteNpcServerCommand,
        Kind::StoryLines,
        Kind::BuyItem,
        Kind::BuyItemResult,
        Kind::RequestCraft,
        Kind::QueryCashShopProductList,
        Kind::CashShopCategoryList,
        Kind::CashShopProductList,
        Kind::QueryCashShopBalance,
        Kind::CashShopBalance,
        Kind::QueryCashShopInventory,
        Kind::CashShopInventory,
        Kind::QueryCashShopItemPickUp,
        Kind::QueryCashShopPurchaseItem,
        Kind::CashShopFail,
        Kind::WishListSelect,
        Kind::WishListSelectResponse,
        Kind::WishListInsert,
        Kind::WishListInsertResponse,
        Kind::WishListDelete,
        Kind::WishListDeleteResponse,
        Kind::BeautyShopCustomize,
        Kind::RequestSecuredOperation,
        Kind::UseExtraStorage,
        Kind::StatusEffectUpdated,
        Kind::RequestSortInventory,
        Kind::SecuredOperation,
        Kind::QueryShopItemInfo,
        Kind::SellItem,
        Kind::QueryBuybackList,
        Kind::BuybackListResult,
        Kind::RepairItem,
        Kind::UpdateStoryGuide,
        Kind::StoryVariables,
        Kind::QueryQuestProgress,
        Kind::QuestProgress,
        Kind::UpdateTitle,
        Kind::HideCostume,
        Kind::TitleList,
        Kind::EnhanceItem,
        Kind::EnhanceItemResult,
        Kind::QueryRestoreItemList,
        Kind::RestoreItemList,
        Kind::EnchantItem,
        Kind::EnchantItemResult,
    ] {
        registered(kind.category(), kind.type_id());
    }
    for profile in profiles {
        let cases = profile["wire_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2370);
        for (n, case) in cases.iter().enumerate() {
            match case["kind"].as_str().unwrap() {
                "update_inventory_info" => check_inventory(Kind::UpdateInventoryInfo, case, n),
                "item_fail" => check_inventory(Kind::ItemFail, case, n),
                "move_inventory_item" => check_inventory(Kind::MoveInventoryItem, case, n),
                "equip_bundle" => check_inventory(Kind::EquipBundle, case, n),
                "quick_slot_info" => check_inventory(Kind::QuickSlotInfo, case, n),
                "query_ap" => check_inventory(Kind::QueryAp, case, n),
                "ap" => check_inventory(Kind::Ap, case, n),
                "use_inventory_item" => check_inventory(Kind::UseInventoryItem, case, n),
                "use_inventory_item_ok" => check_inventory(Kind::UseInventoryItemOk, case, n),
                "query_skill_list" => check_inventory(Kind::QuerySkillList, case, n),
                "skill_list" => check_inventory(Kind::SkillList, case, n),
                "learn_skill" => check_inventory(Kind::LearnSkill, case, n),
                "skill_completion" => check_inventory(Kind::SkillCompletion, case, n),
                "query_common_info" => check_inventory(Kind::QueryCharacterCommonInfo, case, n),
                "learn_new_skill" => check_inventory(Kind::LearnNewSkill, case, n),
                "learn_new_skill_result" => check_inventory(Kind::LearnNewSkillResult, case, n),
                "query_npc_list" => check_inventory(Kind::QueryNpcList, case, n),
                "npc_list" => check_inventory(Kind::NpcList, case, n),
                "query_npc_talk" => check_inventory(Kind::QueryNpcTalk, case, n),
                "npc_talk" => check_inventory(Kind::NpcTalk, case, n),
                "select_button" => check_inventory(Kind::SelectButton, case, n),
                "story_lines" => check_inventory(Kind::StoryLines, case, n),
                "buy_item" => check_inventory(Kind::BuyItem, case, n),
                "request_craft" => check_inventory(Kind::RequestCraft, case, n),
                "query_cash_shop_product_list" => {
                    check_inventory(Kind::QueryCashShopProductList, case, n)
                }
                "cash_shop_category_list" => check_inventory(Kind::CashShopCategoryList, case, n),
                "cash_shop_product_list" => check_inventory(Kind::CashShopProductList, case, n),
                "query_cash_shop_balance" => check_inventory(Kind::QueryCashShopBalance, case, n),
                "cash_shop_balance" => check_inventory(Kind::CashShopBalance, case, n),
                "query_cash_shop_inventory" => {
                    check_inventory(Kind::QueryCashShopInventory, case, n)
                }
                "cash_shop_inventory" => check_inventory(Kind::CashShopInventory, case, n),
                "query_cash_shop_item_pick_up" => {
                    check_inventory(Kind::QueryCashShopItemPickUp, case, n)
                }
                "query_cash_shop_purchase_item" => {
                    check_inventory(Kind::QueryCashShopPurchaseItem, case, n)
                }
                "cash_shop_fail" => check_inventory(Kind::CashShopFail, case, n),
                "wish_list_select" => check_inventory(Kind::WishListSelect, case, n),
                "wish_list_select_response" => {
                    check_inventory(Kind::WishListSelectResponse, case, n)
                }
                "wish_list_insert" => check_inventory(Kind::WishListInsert, case, n),
                "wish_list_insert_response" => {
                    check_inventory(Kind::WishListInsertResponse, case, n)
                }
                "wish_list_delete" => check_inventory(Kind::WishListDelete, case, n),
                "wish_list_delete_response" => {
                    check_inventory(Kind::WishListDeleteResponse, case, n)
                }
                "beauty_shop_customize" => check_inventory(Kind::BeautyShopCustomize, case, n),
                "request_secured_operation" => {
                    check_inventory(Kind::RequestSecuredOperation, case, n)
                }
                "secured_operation" => check_inventory(Kind::SecuredOperation, case, n),
                "use_extra_storage" => check_inventory(Kind::UseExtraStorage, case, n),
                "status_effect_updated" => check_inventory(Kind::StatusEffectUpdated, case, n),
                "request_sort_inventory" => check_inventory(Kind::RequestSortInventory, case, n),
                "buy_item_result" => check_inventory(Kind::BuyItemResult, case, n),
                "query_shop_item_info" => check_inventory(Kind::QueryShopItemInfo, case, n),
                "sell_item" => check_inventory(Kind::SellItem, case, n),
                "query_buyback_list" => check_inventory(Kind::QueryBuybackList, case, n),
                "buyback_list_result" => check_inventory(Kind::BuybackListResult, case, n),
                "repair_item" => check_inventory(Kind::RepairItem, case, n),
                "update_story_guide" => check_inventory(Kind::UpdateStoryGuide, case, n),
                "story_variables" => check_inventory(Kind::StoryVariables, case, n),
                "query_quest_progress" => check_inventory(Kind::QueryQuestProgress, case, n),
                "quest_progress" => check_inventory(Kind::QuestProgress, case, n),
                "update_title" => check_inventory(Kind::UpdateTitle, case, n),
                "set_story_line_status" => check_inventory(Kind::SetStoryLineStatus, case, n),
                "hide_costume" => check_inventory(Kind::HideCostume, case, n),
                "title_list" => check_inventory(Kind::TitleList, case, n),
                "enhance_item" => check_inventory(Kind::EnhanceItem, case, n),
                "enhance_item_result" => check_inventory(Kind::EnhanceItemResult, case, n),
                "query_restore_item_list" => check_inventory(Kind::QueryRestoreItemList, case, n),
                "restore_item_list" => check_inventory(Kind::RestoreItemList, case, n),
                "enchant_item" => check_inventory(Kind::EnchantItem, case, n),
                "enchant_item_result" => check_inventory(Kind::EnchantItemResult, case, n),
                "manufacture_info" => check_inventory(Kind::ManufactureInfo, case, n),
                "manufacture_craft" => check_inventory(Kind::ManufactureCraft, case, n),
                "manufacture_level_up" => check_inventory(Kind::ManufactureLevelUp, case, n),
                "unequippable_parts_info" => check_inventory(Kind::UnequippablePartsInfo, case, n),
                "update_storage_info" => check_inventory(Kind::UpdateStorageInfo, case, n),
                "random_item_reward_list" => check_inventory(Kind::RandomItemRewardList, case, n),
                "skill_enhance_session_start" => {
                    check_inventory(Kind::SkillEnhanceSessionStart, case, n)
                }
                "skill_enhance_session_end" => {
                    check_inventory(Kind::SkillEnhanceSessionEnd, case, n)
                }
                "skill_enhance_use_erg" => check_inventory(Kind::SkillEnhanceUseErg, case, n),
                "skill_enhance_use_erg_result" => {
                    check_inventory(Kind::SkillEnhanceUseErgResult, case, n)
                }
                "skill_enhance" => check_inventory(Kind::SkillEnhance, case, n),
                "skill_enhance_result" => check_inventory(Kind::SkillEnhanceResult, case, n),
                "skill_enhance_changed" => check_inventory(Kind::SkillEnhanceChanged, case, n),
                "skill_enhance_use_durability" => {
                    check_inventory(Kind::SkillEnhanceUseDurability, case, n)
                }
                "skill_enhance_restore_durability" => {
                    check_inventory(Kind::SkillEnhanceRestoreDurability, case, n)
                }
                "max_durability_repair_item" => {
                    check_inventory(Kind::MaxDurabilityRepairItem, case, n)
                }
                "execute_npc_server_command" => {
                    check_inventory(Kind::ExecuteNpcServerCommand, case, n)
                }
                "costume_update" => check_costume(case, n),
                _ => panic!("unknown fixture type"),
            }
        }
    }
}
#[test]
fn item_fail_message_matches_source_dialog_shape() {
    let message = Message::ItemFail(Some(wire::HeroesString {
        format: Some("GameUI_Heroes_EquipItemFail_System".to_owned()),
        parameters: Some(Vec::new()),
    }));
    let body = wire::encode(Kind::ItemFail, Some(&message), DEFAULT_MAX_BODY).unwrap();
    assert!(wire::decode(Kind::ItemFail, &body, DEFAULT_MAX_BODY).unwrap() == Some(message));
}
#[test]
fn client_ingress_ignores_trailing_bytes_like_serialize_reader() {
    use vindictus_server::protocol::inventory::{self as wire, Kind, Message};
    // GUID prefix + non-null marker + one byte the 1.69 EN client appends (18 bytes).
    let mut body = Kind::QueryNpcList.type_id().as_bytes().to_vec();
    body.extend_from_slice(&[1, 0]);
    assert!(matches!(
        wire::decode_ingress(Kind::QueryNpcList, 0, &body, DEFAULT_MAX_BODY),
        Ok(Some(Message::QueryNpcList))
    ));
    // The strict decode used by fixtures and round trips still rejects it.
    assert!(wire::decode(Kind::QueryNpcList, &[1, 0], DEFAULT_MAX_BODY).is_err());
    // A truncated body is still refused.
    assert!(wire::decode_ingress(Kind::QueryNpcTalk, 0, &body[..16], DEFAULT_MAX_BODY).is_err());
}
