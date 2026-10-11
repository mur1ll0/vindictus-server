//! Generated from original ServiceCore.dll metadata on .NET Framework, not invented opcodes.
//! Regenerate with scripts/reverse/generate-registry.py after exporting both platform tables.
//! This identifies the recovered server build; matching the client still requires runtime acceptance.
use super::converter::ClassRegistration;
use uuid::Uuid;
/// SHA256 of the exact server profile.
pub const SOURCE_SHA256: &str = "be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683";
/// Registered nonzero categories in CLR enumeration order.
pub const REGISTERED_CLASSES: &[ClassRegistration] = &[
    // ServiceCore.EndPointNetwork.BurnGaugeRequestMessage
    ClassRegistration {
        category_id: 1,
        type_id: Uuid::from_u128(0xf975bf477fed3ff1803a01d62c72d752),
    },
    // ServiceCore.EndPointNetwork.BurnItemInfo
    ClassRegistration {
        category_id: 2,
        type_id: Uuid::from_u128(0x316dc351432d3b4492a7e0985c8516ab),
    },
    // ServiceCore.EndPointNetwork.BurnItemsMessage
    ClassRegistration {
        category_id: 3,
        type_id: Uuid::from_u128(0xf51531801aa43d67be711b6c819ffd7a),
    },
    // ServiceCore.EndPointNetwork.BurnRewardItemsMessage
    ClassRegistration {
        category_id: 4,
        type_id: Uuid::from_u128(0x72aa79720b9e307a8ae16a0dc6a9cb6c),
    },
    // ServiceCore.EndPointNetwork.BurnResultMessage
    ClassRegistration {
        category_id: 5,
        type_id: Uuid::from_u128(0x94c2eeca52a339caafaabf4eaede7ec6),
    },
    // ServiceCore.EndPointNetwork.BurnJackpotMessage
    ClassRegistration {
        category_id: 6,
        type_id: Uuid::from_u128(0xda0325cce0d53de8a751756bbefff7ee),
    },
    // ServiceCore.EndPointNetwork.BurnGauge
    ClassRegistration {
        category_id: 7,
        type_id: Uuid::from_u128(0x88ee50fb05f93ef98fcf806d508010b4),
    },
    // ServiceCore.EndPointNetwork.Captcha.CaptchaRefreshMessage
    ClassRegistration {
        category_id: 8,
        type_id: Uuid::from_u128(0xf46174acc02634d3a11e69f8672ebc3d),
    },
    // ServiceCore.EndPointNetwork.Captcha.CaptchaRequestMessage
    ClassRegistration {
        category_id: 9,
        type_id: Uuid::from_u128(0xbc9d6925b0da3137ab9547a30a9fcbd6),
    },
    // ServiceCore.EndPointNetwork.Captcha.CaptchaResponseMessage
    ClassRegistration {
        category_id: 10,
        type_id: Uuid::from_u128(0xeec568bbe9fd3359b1d3056434a0534c),
    },
    // ServiceCore.EndPointNetwork.Captcha.CaptchaResponseResultMessage
    ClassRegistration {
        category_id: 11,
        type_id: Uuid::from_u128(0x6ea9e929023d38febd13bf3c85e47efa),
    },
    // ServiceCore.EndPointNetwork.ChangedCashShopMessage
    ClassRegistration {
        category_id: 12,
        type_id: Uuid::from_u128(0x2b83b47e2e673c50a0c78b404c0fadf7),
    },
    // ServiceCore.EndPointNetwork.GiveAPMessage
    ClassRegistration {
        category_id: 13,
        type_id: Uuid::from_u128(0xd781c3046ccc3d31980142b4e1ee09e5),
    },
    // ServiceCore.EndPointNetwork.Item.NotifyBurnMessage
    ClassRegistration {
        category_id: 14,
        type_id: Uuid::from_u128(0xb61bf401746832bdaefd2b4e62a8a073),
    },
    // ServiceCore.EndPointNetwork.Item.NotifyRandomItemMessage
    ClassRegistration {
        category_id: 15,
        type_id: Uuid::from_u128(0x2638f35abac938988c3c15daa673e2fd),
    },
    // ServiceCore.EndPointNetwork.NGSecurityMessage
    ClassRegistration {
        category_id: 16,
        type_id: Uuid::from_u128(0x5ee15fa278083ab28dd2caa46efd8124),
    },
    // ServiceCore.EndPointNetwork.UpdateFlexibleMapLoadingInfoMessage
    ClassRegistration {
        category_id: 17,
        type_id: Uuid::from_u128(0x17b1e1f8a9863e139df52838b4f9a955),
    },
    // ServiceCore.EndPointNetwork.UseInventoryItemWithCountMessage
    ClassRegistration {
        category_id: 18,
        type_id: Uuid::from_u128(0x217592889a2a365f8c993327d52d7611),
    },
    // ServiceCore.EndPointNetwork.EnhanceSuccessRatioDebugMessage
    ClassRegistration {
        category_id: 19,
        type_id: Uuid::from_u128(0xd997386eea103f25a9b693c7b6d69429),
    },
    // ServiceCore.EndPointNetwork.HotSpringRequestInfoMessage
    ClassRegistration {
        category_id: 20,
        type_id: Uuid::from_u128(0x7b268fff460d3196ad201cd3884c57d4),
    },
    // ServiceCore.EndPointNetwork.HotSpringPotionEffectInfo
    ClassRegistration {
        category_id: 21,
        type_id: Uuid::from_u128(0x5005dbbe58503ccc909abd767d6401ef),
    },
    // ServiceCore.EndPointNetwork.HotSpringRequestInfoResultMessage
    ClassRegistration {
        category_id: 22,
        type_id: Uuid::from_u128(0x97acc6175f413e3a948ddd1d6bb89fc9),
    },
    // ServiceCore.EndPointNetwork.HotSpringRequestInfoResultChannelMessage
    ClassRegistration {
        category_id: 23,
        type_id: Uuid::from_u128(0xffa461f7ea1f31ea8b38b015b596103c),
    },
    // ServiceCore.EndPointNetwork.HotSpringAddPotionMessage
    ClassRegistration {
        category_id: 24,
        type_id: Uuid::from_u128(0xf26d5bf9e2323b1798b06122c15a518e),
    },
    // ServiceCore.EndPointNetwork.HotSpringAddPotionResultMessage
    ClassRegistration {
        category_id: 25,
        type_id: Uuid::from_u128(0x10296e17087735159f51ff3cdb908436),
    },
    // ServiceCore.EndPointNetwork.HotSpringRequestPotionEffectMessage
    ClassRegistration {
        category_id: 26,
        type_id: Uuid::from_u128(0x156a9b52538c3ca9b30d9ecc5ae1d2bc),
    },
    // ServiceCore.EndPointNetwork.Item.BraceletCombinationResultMessage
    ClassRegistration {
        category_id: 27,
        type_id: Uuid::from_u128(0xb04ed2b64d083fbd9fb759ba85f0a481),
    },
    // ServiceCore.EndPointNetwork.EnchantLimitlessMessage
    ClassRegistration {
        category_id: 28,
        type_id: Uuid::from_u128(0x630f2ead34d6339791f158a0c5f8ce66),
    },
    // ServiceCore.EndPointNetwork.NotifyEnhanceMessage
    ClassRegistration {
        category_id: 29,
        type_id: Uuid::from_u128(0xd12c115b694d3c648305adf13132d246),
    },
    // ServiceCore.EndPointNetwork.Item.RequestItemCombinationMessage
    ClassRegistration {
        category_id: 30,
        type_id: Uuid::from_u128(0x6f5039cc7b683663a8407e815d473369),
    },
    // ServiceCore.EndPointNetwork.Item.RequestPartChangingMessage
    ClassRegistration {
        category_id: 31,
        type_id: Uuid::from_u128(0xb465a74fe2903d368e9d6b5891caa627),
    },
    // ServiceCore.EndPointNetwork.Item.ItemCombinationResultMessage
    ClassRegistration {
        category_id: 32,
        type_id: Uuid::from_u128(0xa17b6249902f376c9b9b8c37a41b6a2d),
    },
    // ServiceCore.EndPointNetwork.Item.RequestBraceletCombinationMessage
    ClassRegistration {
        category_id: 33,
        type_id: Uuid::from_u128(0x8a3dc6cb742b3512a78b125d6f6139c0),
    },
    // ServiceCore.EndPointNetwork.Item.RequestGemstoneRollbackMessage
    ClassRegistration {
        category_id: 34,
        type_id: Uuid::from_u128(0x4a3c2168df45385abb70e87ddd461f38),
    },
    // ServiceCore.EndPointNetwork.MakeNamedRingMessage
    ClassRegistration {
        category_id: 35,
        type_id: Uuid::from_u128(0xfab6db0c879831bd831a7923fb4fe4ed),
    },
    // ServiceCore.EndPointNetwork.Item.UpdateHistoryBookMessage
    ClassRegistration {
        category_id: 36,
        type_id: Uuid::from_u128(0xecc2930048393bd9a561c286f464b5e3),
    },
    // ServiceCore.EndPointNetwork.RequestMarbleInfoMessage
    ClassRegistration {
        category_id: 37,
        type_id: Uuid::from_u128(0x63093f2239073ef89fe49d6dac4fa859),
    },
    // ServiceCore.EndPointNetwork.MarbleInfoResultMessage
    ClassRegistration {
        category_id: 38,
        type_id: Uuid::from_u128(0x34f81f9deb42347fb658a538c04926cc),
    },
    // ServiceCore.EndPointNetwork.RequestMarbleCastDiceMessage
    ClassRegistration {
        category_id: 39,
        type_id: Uuid::from_u128(0x84eb18d5b11132ba8b3734727f8017da),
    },
    // ServiceCore.EndPointNetwork.MarbleCastDiceResultMessage
    ClassRegistration {
        category_id: 40,
        type_id: Uuid::from_u128(0x556b8a2fbf263bf1aa8b3d1083d5e11a),
    },
    // ServiceCore.EndPointNetwork.RequestMarbleProcessNodeMessage
    ClassRegistration {
        category_id: 41,
        type_id: Uuid::from_u128(0x18231ddc48d033a98b1039dd7f104713),
    },
    // ServiceCore.EndPointNetwork.MarbleProcessNodeResultMessage
    ClassRegistration {
        category_id: 42,
        type_id: Uuid::from_u128(0xa32b7179c5a236c9a5df96f0e3ef1475),
    },
    // ServiceCore.EndPointNetwork.RequestMarbleProcessChanceRoadMessage
    ClassRegistration {
        category_id: 43,
        type_id: Uuid::from_u128(0x31c0f9a3a0c43be5aa019451a41f17fd),
    },
    // ServiceCore.EndPointNetwork.MarbleResultMessage
    ClassRegistration {
        category_id: 44,
        type_id: Uuid::from_u128(0xecf6e200b1b13d32810261de95880379),
    },
    // ServiceCore.EndPointNetwork.MarbleSetTimerMessage
    ClassRegistration {
        category_id: 45,
        type_id: Uuid::from_u128(0x34d75e869ef73eacba62ab61d3220c18),
    },
    // ServiceCore.EndPointNetwork.MarbleGetItemMessage
    ClassRegistration {
        category_id: 46,
        type_id: Uuid::from_u128(0x7546ede05f3335508796fe921dfcabd8),
    },
    // ServiceCore.EndPointNetwork.MicroPlay.AllUserJoinCompleteMessage
    ClassRegistration {
        category_id: 47,
        type_id: Uuid::from_u128(0x3f79e7b849d534f28970b8b5b47aa1eb),
    },
    // ServiceCore.EndPointNetwork.InsertBlessStoneCompleteMessage
    ClassRegistration {
        category_id: 48,
        type_id: Uuid::from_u128(0x915f89131565340c99ca619e37ffd1f7),
    },
    // ServiceCore.EndPointNetwork.InsertBlessStoneMessage
    ClassRegistration {
        category_id: 49,
        type_id: Uuid::from_u128(0x96b29c587e72335297e990941d630ac5),
    },
    // ServiceCore.EndPointNetwork.QuerySharedInventoryMessage
    ClassRegistration {
        category_id: 50,
        type_id: Uuid::from_u128(0x2340ef4eb3f83cd0b4ebb4fa33702c61),
    },
    // ServiceCore.EndPointNetwork.RequestAddPeerMessage
    ClassRegistration {
        category_id: 51,
        type_id: Uuid::from_u128(0x34bb53a9c5ef38eb963254d4c3102fea),
    },
    // ServiceCore.EndPointNetwork.AllUserGoalEventMessage
    ClassRegistration {
        category_id: 52,
        type_id: Uuid::from_u128(0x8531c0953d0a3e5f8e05d60f0c5ade95),
    },
    // ServiceCore.EndPointNetwork.AllUserGoalEventModifyMessage
    ClassRegistration {
        category_id: 53,
        type_id: Uuid::from_u128(0xad82781dee6738c29674626448d02d94),
    },
    // ServiceCore.EndPointNetwork.AllUserGoalRewardMessage
    ClassRegistration {
        category_id: 54,
        type_id: Uuid::from_u128(0x22836c87a7f6333baa9cdd0987da4bb6),
    },
    // ServiceCore.EndPointNetwork.AltarStatusEffectMessage
    ClassRegistration {
        category_id: 55,
        type_id: Uuid::from_u128(0xbd6a5b8838053965a5faa7180f638ecb),
    },
    // ServiceCore.EndPointNetwork.QueryAvatarSynthesisMaterialRecipesMessage
    ClassRegistration {
        category_id: 56,
        type_id: Uuid::from_u128(0x1fc84a4485103599854ec648902d4174),
    },
    // ServiceCore.EndPointNetwork.AvatarSynthesisMaterialRecipesMessage
    ClassRegistration {
        category_id: 57,
        type_id: Uuid::from_u128(0xd8f103f9cfea321a930f88437f943f7c),
    },
    // ServiceCore.EndPointNetwork.AvatarSynthesisItemMessage
    ClassRegistration {
        category_id: 58,
        type_id: Uuid::from_u128(0x8518472d0f5938a98da4a29380e97051),
    },
    // ServiceCore.EndPointNetwork.AvatarSynthesisRequestMessage
    ClassRegistration {
        category_id: 59,
        type_id: Uuid::from_u128(0x75fb1aba7a383da0a93f89550d289230),
    },
    // ServiceCore.EndPointNetwork.AvatarSynthesisResultMessage
    ClassRegistration {
        category_id: 60,
        type_id: Uuid::from_u128(0x2a63b6e26e513fb3ac47fb0ab8f4987d),
    },
    // ServiceCore.EndPointNetwork.CashshopTirCoinResultMessage
    ClassRegistration {
        category_id: 61,
        type_id: Uuid::from_u128(0xc2b788a9c3c535ae85db0096f8eaf6f3),
    },
    // ServiceCore.EndPointNetwork.CIDByNameMessage
    ClassRegistration {
        category_id: 62,
        type_id: Uuid::from_u128(0xd6e7b2ef3abc31bdbceee9dd69f16a61),
    },
    // ServiceCore.EndPointNetwork.CounterEventMessage
    ClassRegistration {
        category_id: 63,
        type_id: Uuid::from_u128(0x3e1c2155f8783e20a2bf72d5171975f2),
    },
    // ServiceCore.EndPointNetwork.DecomposeItemMessage
    ClassRegistration {
        category_id: 64,
        type_id: Uuid::from_u128(0x29250f9fcfa73c3b83e89d81faa58ede),
    },
    // ServiceCore.EndPointNetwork.DecomposeItemResultMessage
    ClassRegistration {
        category_id: 65,
        type_id: Uuid::from_u128(0x47016df8edc230cd876978de2991587e),
    },
    // ServiceCore.EndPointNetwork.ExchangeMileageMessage
    ClassRegistration {
        category_id: 66,
        type_id: Uuid::from_u128(0xa3cece80d7c7362bb7e5e1f4de482abb),
    },
    // ServiceCore.EndPointNetwork.ExchangeMileageResultMessage
    ClassRegistration {
        category_id: 67,
        type_id: Uuid::from_u128(0x108f2d24a96f306ab1173a35b1f8ce15),
    },
    // ServiceCore.EndPointNetwork.FreeTitleNameCheckMessage
    ClassRegistration {
        category_id: 68,
        type_id: Uuid::from_u128(0x5285976222773e31a68bd679d0c98703),
    },
    // ServiceCore.EndPointNetwork.FreeTitleNameCheckResultMessage
    ClassRegistration {
        category_id: 69,
        type_id: Uuid::from_u128(0x75d5d9e2358e32b19432384cef8400bd),
    },
    // ServiceCore.EndPointNetwork.UseFreeTitleMessage
    ClassRegistration {
        category_id: 70,
        type_id: Uuid::from_u128(0xe1b20ebf959a3700849996f1166287d7),
    },
    // ServiceCore.EndPointNetwork.GetUserIDMessage
    ClassRegistration {
        category_id: 71,
        type_id: Uuid::from_u128(0xcc6186be71f834f19bd2cc51eecc5fa7),
    },
    // ServiceCore.EndPointNetwork.UserIDMessage
    ClassRegistration {
        category_id: 72,
        type_id: Uuid::from_u128(0x6cc74c8f18ee3256922f742060c6df19),
    },
    // ServiceCore.EndPointNetwork.AddFriendshipInfoMessage
    ClassRegistration {
        category_id: 73,
        type_id: Uuid::from_u128(0xaa669f33b1e939408a4c932f496b329e),
    },
    // ServiceCore.EndPointNetwork.GetFriendshipInfoMessage
    ClassRegistration {
        category_id: 74,
        type_id: Uuid::from_u128(0xe9508ab90a063e15a38cfd25895841d8),
    },
    // ServiceCore.EndPointNetwork.FriendshipInfoListMessage
    ClassRegistration {
        category_id: 75,
        type_id: Uuid::from_u128(0x03ab803d72323bf7b30dd62d9411dcdf),
    },
    // ServiceCore.EndPointNetwork.DeleteFriendshipInfoMessage
    ClassRegistration {
        category_id: 76,
        type_id: Uuid::from_u128(0x6bd8d31a0d70339794c426faca65401d),
    },
    // ServiceCore.EndPointNetwork.GetFriendshipPointMessage
    ClassRegistration {
        category_id: 77,
        type_id: Uuid::from_u128(0xbb81a0152e133ba487917e1af4e3c3e3),
    },
    // ServiceCore.EndPointNetwork.UpdateFriendshipPointMessage
    ClassRegistration {
        category_id: 78,
        type_id: Uuid::from_u128(0xc73952dd94ac30ec99f6811d7adea12e),
    },
    // ServiceCore.EndPointNetwork.AddFriendShipMessage
    ClassRegistration {
        category_id: 79,
        type_id: Uuid::from_u128(0x14886875dafc3565a1faaa2c0f0cdd4c),
    },
    // ServiceCore.EndPointNetwork.AddFriendShipResultMessage
    ClassRegistration {
        category_id: 80,
        type_id: Uuid::from_u128(0x13fb868f6cd13b968d96714dfefa6203),
    },
    // ServiceCore.EndPointNetwork.GatheringMessage
    ClassRegistration {
        category_id: 81,
        type_id: Uuid::from_u128(0x3410403541303d7d9af51ef6188d2dd4),
    },
    // ServiceCore.EndPointNetwork.GameResourceRequestMessage
    ClassRegistration {
        category_id: 82,
        type_id: Uuid::from_u128(0xd156e5db264333ad933e2e4cf0693e05),
    },
    // ServiceCore.EndPointNetwork.GameResourceRespondMessage
    ClassRegistration {
        category_id: 83,
        type_id: Uuid::from_u128(0x5c6a08dbecb53731a7a420002736909c),
    },
    // ServiceCore.EndPointNetwork.GuildService.ChangeMasterMessage
    ClassRegistration {
        category_id: 84,
        type_id: Uuid::from_u128(0x8f63544872bd3685bceab6cd22d0db8d),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildChatMemberInfoMessage
    ClassRegistration {
        category_id: 85,
        type_id: Uuid::from_u128(0x95a85ad2b27234229b923a664dc0cba9),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildChatMessage
    ClassRegistration {
        category_id: 86,
        type_id: Uuid::from_u128(0x76a1e90829cf3ecd82759453e1534766),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildInventoryInfoMessage
    ClassRegistration {
        category_id: 87,
        type_id: Uuid::from_u128(0x58f0a03b7aae3521bb107c39acc518e0),
    },
    // ServiceCore.EndPointNetwork.GuildService.JoinGuildChatRoomMessage
    ClassRegistration {
        category_id: 88,
        type_id: Uuid::from_u128(0x6ea314db467536a1979ad0942107da04),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildGainGPMessage
    ClassRegistration {
        category_id: 89,
        type_id: Uuid::from_u128(0xd829f2cbbaba3bbf828bcab4fa8aaae4),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildLevelUpMessage
    ClassRegistration {
        category_id: 90,
        type_id: Uuid::from_u128(0x2d17a07699cb33c58effd05926933c7a),
    },
    // ServiceCore.EndPointNetwork.Item.BingoBoardResultMessage
    ClassRegistration {
        category_id: 91,
        type_id: Uuid::from_u128(0x50ec83991eec35f191e998f63a12d244),
    },
    // ServiceCore.EndPointNetwork.Housing.EndPendingDialogMessage
    ClassRegistration {
        category_id: 92,
        type_id: Uuid::from_u128(0x993acf8589e53fd9a1ef743ed1e7273b),
    },
    // ServiceCore.EndPointNetwork.Housing.EnterHousingMessage
    ClassRegistration {
        category_id: 93,
        type_id: Uuid::from_u128(0xe390c24a94d23ad69cf668e1f7a60289),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingHostRestartingMessage
    ClassRegistration {
        category_id: 94,
        type_id: Uuid::from_u128(0xc40209a252f13069902aef3a0e2ced2c),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingInvitationRejectMessage
    ClassRegistration {
        category_id: 95,
        type_id: Uuid::from_u128(0x20ef7e9c4724398c9bc8c6f94606ba07),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingInvitedMessage
    ClassRegistration {
        category_id: 96,
        type_id: Uuid::from_u128(0xeb134630a7cf387a8410505a54eaeafe),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingListMessage
    ClassRegistration {
        category_id: 97,
        type_id: Uuid::from_u128(0xefa9829d4b5136f2a41cc10a3e40c1f6),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingPartyInfoMessage
    ClassRegistration {
        category_id: 98,
        type_id: Uuid::from_u128(0x71d67e6a11b23b56ab0453f34c8a8d76),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingRoomListMessage
    ClassRegistration {
        category_id: 99,
        type_id: Uuid::from_u128(0x4105d883c1523f13b4158abfa1ece08b),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingStartGrantedMessage
    ClassRegistration {
        category_id: 100,
        type_id: Uuid::from_u128(0x0194b3ef506030298b28731461ae1b7c),
    },
    // ServiceCore.EndPointNetwork.Housing.QueryHousingItemsMessage
    ClassRegistration {
        category_id: 101,
        type_id: Uuid::from_u128(0x83ab7f7c26413b07b5476684533ec4a6),
    },
    // ServiceCore.EndPointNetwork.Housing.QueryHousingListMessage
    ClassRegistration {
        category_id: 102,
        type_id: Uuid::from_u128(0xa472c16093a03756943a00cbb828a93f),
    },
    // ServiceCore.EndPointNetwork.Housing.QueryHousingPropsMessage
    ClassRegistration {
        category_id: 103,
        type_id: Uuid::from_u128(0x039e7adb42253bee90f5328cf874f726),
    },
    // ServiceCore.EndPointNetwork.Housing.SaveHousingPropsMessage
    ClassRegistration {
        category_id: 104,
        type_id: Uuid::from_u128(0x8adff0b8f6ff3c2f8ee58db1ecdbad3c),
    },
    // ServiceCore.EndPointNetwork.Housing.SearchHousingRoomMessage
    ClassRegistration {
        category_id: 105,
        type_id: Uuid::from_u128(0x234812e7e2c5343688c9b7685a68c4dc),
    },
    // ServiceCore.EndPointNetwork.Housing.SwapHousingItemMessage
    ClassRegistration {
        category_id: 106,
        type_id: Uuid::from_u128(0xbb199f0356603224b1031583a28a1ad9),
    },
    // ServiceCore.EndPointNetwork.Housing.UpdateHousingItemsMessage
    ClassRegistration {
        category_id: 107,
        type_id: Uuid::from_u128(0x888333c6f9fe3baea4a74c64be93c4d0),
    },
    // ServiceCore.EndPointNetwork.Housing.UpdateHousingPropsMessage
    ClassRegistration {
        category_id: 108,
        type_id: Uuid::from_u128(0x35c2ac0c22683acdb9ff5e3725902a84),
    },
    // ServiceCore.EndPointNetwork.Housing.CreateHousingMessage
    ClassRegistration {
        category_id: 109,
        type_id: Uuid::from_u128(0xb25e6b26ed9d3167a4adab9a57acfdce),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingGameHostedMessage
    ClassRegistration {
        category_id: 110,
        type_id: Uuid::from_u128(0xf0b562e9417130e0b84acfbb2796f6cf),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingHostInfoMessage
    ClassRegistration {
        category_id: 111,
        type_id: Uuid::from_u128(0x98a2f206541f3aa8972c96daf3129858),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingKickMessage
    ClassRegistration {
        category_id: 112,
        type_id: Uuid::from_u128(0xbf10c2329dc637999f5a67be18d6c529),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingKickedMessage
    ClassRegistration {
        category_id: 113,
        type_id: Uuid::from_u128(0xcfa16fb1f72d3a5fbb3dfc0c2a5ffbab),
    },
    // ServiceCore.EndPointNetwork.Housing.HousingMemberInfoMessage
    ClassRegistration {
        category_id: 114,
        type_id: Uuid::from_u128(0x4e5a25c1f8623aabaeab82b8664759cf),
    },
    // ServiceCore.EndPointNetwork.Housing.JoinHousingMessage
    ClassRegistration {
        category_id: 115,
        type_id: Uuid::from_u128(0xbb18494fed03314fae35b83f85bbc8b0),
    },
    // ServiceCore.EndPointNetwork.Housing.LeaveHousingMessage
    ClassRegistration {
        category_id: 116,
        type_id: Uuid::from_u128(0xe4e4df838f243fd2bfccaeab3c742d4b),
    },
    // ServiceCore.EndPointNetwork.Item.AttendanceInfoMessage
    ClassRegistration {
        category_id: 117,
        type_id: Uuid::from_u128(0xa13b8bd0cba73b8c98cd93927e80abea),
    },
    // ServiceCore.EndPointNetwork.Item.GiveCashShopDiscountCouponMessage
    ClassRegistration {
        category_id: 118,
        type_id: Uuid::from_u128(0xe50899d980d7304cac2b7604353ea686),
    },
    // ServiceCore.EndPointNetwork.Item.BuyIngameCashshopUseTirMessage
    ClassRegistration {
        category_id: 119,
        type_id: Uuid::from_u128(0xa2c102aaaf0c332aa435cf0cd19c2739),
    },
    // ServiceCore.EndPointNetwork.Item.BuyIngameCashshopUseTirResultMessage
    ClassRegistration {
        category_id: 120,
        type_id: Uuid::from_u128(0xcaef77a47a8c3a0a8249cc23b0b4177c),
    },
    // ServiceCore.EndPointNetwork.Item.GiveCashShopDiscountCouponResultMessage
    ClassRegistration {
        category_id: 121,
        type_id: Uuid::from_u128(0x4999ce614aa034b09e695edac3330640),
    },
    // ServiceCore.EndPointNetwork.Item.RequestAttendanceRewardMessage
    ClassRegistration {
        category_id: 122,
        type_id: Uuid::from_u128(0x995ae4f22b7e3db98fbebe06811392dd),
    },
    // ServiceCore.EndPointNetwork.Item.SelectTargetItemMessage
    ClassRegistration {
        category_id: 123,
        type_id: Uuid::from_u128(0x34ef22c268ba35479acfa50d46ef337f),
    },
    // ServiceCore.EndPointNetwork.Item.TirCoinInfoMessage
    ClassRegistration {
        category_id: 124,
        type_id: Uuid::from_u128(0x60663598c5bd3b1cb301f71096a5067e),
    },
    // ServiceCore.EndPointNetwork.Item.UseAdvancedFeatherMessage
    ClassRegistration {
        category_id: 125,
        type_id: Uuid::from_u128(0x57edcd5e27043e0fbb44c3cb1da0a72f),
    },
    // ServiceCore.EndPointNetwork.MicroPlay.AnswerFinishQuestMessage
    ClassRegistration {
        category_id: 126,
        type_id: Uuid::from_u128(0x494f0a4aedb435bd9222bf40cd6ec036),
    },
    // ServiceCore.EndPointNetwork.MicroPlay.AskFinishQuestMessage
    ClassRegistration {
        category_id: 127,
        type_id: Uuid::from_u128(0xbda98963a521303bbf6e9d8636832a75),
    },
    // ServiceCore.EndPointNetwork.OpenCustomDialogUIMessage
    ClassRegistration {
        category_id: 128,
        type_id: Uuid::from_u128(0x090fd1c08d833973ab3fb50bc25e65cf),
    },
    // ServiceCore.EndPointNetwork.Pvp.PvpChannelListMessage
    ClassRegistration {
        category_id: 129,
        type_id: Uuid::from_u128(0xa6711fc73b8b35c6a8a52a2d222a9b41),
    },
    // ServiceCore.EndPointNetwork.Pvp.PvpQueryChannelListMessage
    ClassRegistration {
        category_id: 130,
        type_id: Uuid::from_u128(0x28e86f3f74a23fc0a9ae860806aafb63),
    },
    // ServiceCore.EndPointNetwork.QueryBattleInventoryInTownMessage
    ClassRegistration {
        category_id: 131,
        type_id: Uuid::from_u128(0x8ec155f35b4c3dad8b695df2b3f376a7),
    },
    // ServiceCore.EndPointNetwork.DSHostConnectionQuery
    ClassRegistration {
        category_id: 132,
        type_id: Uuid::from_u128(0x5b87e60048803535804daa70934b16a7),
    },
    // ServiceCore.EndPointNetwork.QueryFeverPointMessage
    ClassRegistration {
        category_id: 133,
        type_id: Uuid::from_u128(0xd47623531ceb3351b0318e414528a432),
    },
    // ServiceCore.EndPointNetwork.QueryMileagePointMessage
    ClassRegistration {
        category_id: 134,
        type_id: Uuid::from_u128(0x7fdb51a5ddcf35358d2a7f15dfe3a8b6),
    },
    // ServiceCore.EndPointNetwork.RecommendFriendListMessage
    ClassRegistration {
        category_id: 135,
        type_id: Uuid::from_u128(0x3b6dbdb9fbfb3d1eb06bc74df70b9df1),
    },
    // ServiceCore.EndPointNetwork.QuestTimerInfoMessage
    ClassRegistration {
        category_id: 136,
        type_id: Uuid::from_u128(0xb8ae1f90db3f3c7499941c93c83d4a1b),
    },
    // ServiceCore.EndPointNetwork.OpenInGameCashShopUIMessage
    ClassRegistration {
        category_id: 137,
        type_id: Uuid::from_u128(0xd322f4534cbd3e568cdb4f0221d5fc8a),
    },
    // ServiceCore.EndPointNetwork.OpenRandomBoxMessage
    ClassRegistration {
        category_id: 138,
        type_id: Uuid::from_u128(0xdea2a0b100393226b0be55766961559c),
    },
    // ServiceCore.EndPointNetwork.OpenTiticoreUIMessage
    ClassRegistration {
        category_id: 139,
        type_id: Uuid::from_u128(0x06bf6e2ea3aa39cea88adc9efa70214b),
    },
    // ServiceCore.EndPointNetwork.OpenTreasureBoxMessage
    ClassRegistration {
        category_id: 140,
        type_id: Uuid::from_u128(0x60c41902e6613758b68afb8e5eb07a14),
    },
    // ServiceCore.EndPointNetwork.OpenTreasureBoxResultMessage
    ClassRegistration {
        category_id: 141,
        type_id: Uuid::from_u128(0xb7866fc3004938d099ce4479100bd664),
    },
    // ServiceCore.EndPointNetwork.PvpInfoMessage
    ClassRegistration {
        category_id: 142,
        type_id: Uuid::from_u128(0xb7f014ee3d053444ba26f630dec024cd),
    },
    // ServiceCore.EndPointNetwork.Pvp.PvpConfirmJoinMessage
    ClassRegistration {
        category_id: 143,
        type_id: Uuid::from_u128(0xdd1dab96b0423e0ab622d09b812d90c5),
    },
    // ServiceCore.EndPointNetwork.QueryCIDByNameMessage
    ClassRegistration {
        category_id: 144,
        type_id: Uuid::from_u128(0xd29aace4ea1335bd8de5ecbba29a7539),
    },
    // ServiceCore.EndPointNetwork.QueryTiticoreDisplayItemsMessage
    ClassRegistration {
        category_id: 145,
        type_id: Uuid::from_u128(0x65f9d503d9283e50b45c702aa5ea96d6),
    },
    // ServiceCore.EndPointNetwork.IncreasePvpRankMessage
    ClassRegistration {
        category_id: 146,
        type_id: Uuid::from_u128(0x64a4fff0a4cb367db9921c4f5628f5a8),
    },
    // ServiceCore.EndPointNetwork.QueryRandomRankInfoMessage
    ClassRegistration {
        category_id: 147,
        type_id: Uuid::from_u128(0x051924bee16a31729a8b0e65b13edb6f),
    },
    // ServiceCore.EndPointNetwork.RandomRankInfoMessage
    ClassRegistration {
        category_id: 148,
        type_id: Uuid::from_u128(0x7411ad568be63a81bdd3a69f242c3fe2),
    },
    // ServiceCore.EndPointNetwork.RequestBingoRewardMessage
    ClassRegistration {
        category_id: 149,
        type_id: Uuid::from_u128(0xd41b01cdc13b3418b1e5984aa430a71a),
    },
    // ServiceCore.EndPointNetwork.CheckCharacterNameResultMessage
    ClassRegistration {
        category_id: 150,
        type_id: Uuid::from_u128(0x6de1bf749845344890af230ce1779c2b),
    },
    // ServiceCore.EndPointNetwork.RequestJoinPartyMessage
    ClassRegistration {
        category_id: 151,
        type_id: Uuid::from_u128(0x389ed5eb46953b00ab0eb4819c2fe44c),
    },
    // ServiceCore.EndPointNetwork.QueryRestoreItemListMessage
    ClassRegistration {
        category_id: 152,
        type_id: Uuid::from_u128(0xa91b73372cad3eae8175e1763f2413ee),
    },
    // ServiceCore.EndPointNetwork.RestoreItemInfo
    ClassRegistration {
        category_id: 153,
        type_id: Uuid::from_u128(0xcd068e5b2e283868b327881b7682b5fe),
    },
    // ServiceCore.EndPointNetwork.RestoreItemListMessage
    ClassRegistration {
        category_id: 154,
        type_id: Uuid::from_u128(0xb80b34deba173cef9ae3577cfd045708),
    },
    // ServiceCore.EndPointNetwork.RequestRouletteBoardMessage
    ClassRegistration {
        category_id: 155,
        type_id: Uuid::from_u128(0xb471ffaa58d437ce95eb4222fb65ba4f),
    },
    // ServiceCore.EndPointNetwork.RouletteBoardResultMessage
    ClassRegistration {
        category_id: 156,
        type_id: Uuid::from_u128(0x5b2c50f8b32839578ac7574a74f574fe),
    },
    // ServiceCore.EndPointNetwork.RequestRoulettePickSlotMessage
    ClassRegistration {
        category_id: 157,
        type_id: Uuid::from_u128(0xf6e0be65d4a936fea06f3dc1f6ffe301),
    },
    // ServiceCore.EndPointNetwork.RoulettePickSlotResultMessage
    ClassRegistration {
        category_id: 158,
        type_id: Uuid::from_u128(0x5e14d4842f1535649e48931b7cad97d4),
    },
    // ServiceCore.EndPointNetwork.SelectPatternMessage
    ClassRegistration {
        category_id: 159,
        type_id: Uuid::from_u128(0x34362a0a0eab350c9ca8893fdeace15c),
    },
    // ServiceCore.EndPointNetwork.SetFeverPointMessage
    ClassRegistration {
        category_id: 160,
        type_id: Uuid::from_u128(0xf10b19ee4ca7350daf429c6a14725aee),
    },
    // ServiceCore.EndPointNetwork.SetFreeTitleNameMessage
    ClassRegistration {
        category_id: 161,
        type_id: Uuid::from_u128(0x3d1b26880f5a3c3ebccea1dbf9003b99),
    },
    // ServiceCore.EndPointNetwork.SetMileagePointMessage
    ClassRegistration {
        category_id: 162,
        type_id: Uuid::from_u128(0xf0ce12730a5e3ad6aaf7df4ff3a07637),
    },
    // ServiceCore.EndPointNetwork.MoveSharedItemMessage
    ClassRegistration {
        category_id: 163,
        type_id: Uuid::from_u128(0xbcc7805b07a83fab8b54a31d0502908b),
    },
    // ServiceCore.EndPointNetwork.CouponShopItemInfoResultMessage
    ClassRegistration {
        category_id: 164,
        type_id: Uuid::from_u128(0xfa21557002da31519471c410aaebcdb9),
    },
    // ServiceCore.EndPointNetwork.CouponShopItemInfoQueryMessage
    ClassRegistration {
        category_id: 165,
        type_id: Uuid::from_u128(0xee913526555037358cce2157cafa92b7),
    },
    // ServiceCore.EndPointNetwork.ShopItemInfoResultMessage
    ClassRegistration {
        category_id: 166,
        type_id: Uuid::from_u128(0x60137a8e431c307badda79b8ea10f560),
    },
    // ServiceCore.EndPointNetwork.QueryShopItemInfoMessage
    ClassRegistration {
        category_id: 167,
        type_id: Uuid::from_u128(0x4088bd54bc813398838661bdaf5917c1),
    },
    // ServiceCore.EndPointNetwork.ShowCharacterNameChangeDialogMessage
    ClassRegistration {
        category_id: 168,
        type_id: Uuid::from_u128(0xedc69a2a9a0a3c329cde7ec34ebb8106),
    },
    // ServiceCore.EndPointNetwork.ShowTreasureBoxMessage
    ClassRegistration {
        category_id: 169,
        type_id: Uuid::from_u128(0xc0d284be10433600bd6562c8ff36307f),
    },
    // ServiceCore.EndPointNetwork.RespawnMessage
    ClassRegistration {
        category_id: 170,
        type_id: Uuid::from_u128(0xfd334acee87934ed8751b5e6e168bef1),
    },
    // ServiceCore.EndPointNetwork.SetFacebookAccessTokenMessage
    ClassRegistration {
        category_id: 171,
        type_id: Uuid::from_u128(0x7df48ff28a9c3cddad5d4bc47cc42bbe),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceChangedMessage
    ClassRegistration {
        category_id: 172,
        type_id: Uuid::from_u128(0xedfa93968cdf3ccf811f7b68c28731a6),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceSessionStartMessage
    ClassRegistration {
        category_id: 173,
        type_id: Uuid::from_u128(0xc48535b38e5e3483899d0fa1c34cf886),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceSessionEndMessage
    ClassRegistration {
        category_id: 174,
        type_id: Uuid::from_u128(0x7b5daeb1f89236a492f8f7f5ab24babe),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceUseErgMessage
    ClassRegistration {
        category_id: 175,
        type_id: Uuid::from_u128(0x8a2ad99d4c453e7a8c2738c56dc45cda),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceUseErgResultMessage
    ClassRegistration {
        category_id: 176,
        type_id: Uuid::from_u128(0x5a41f49fbf503f92806cc16778a0d731),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceMessage
    ClassRegistration {
        category_id: 177,
        type_id: Uuid::from_u128(0x852e561d12313b4eb53bd8bec2c34d7e),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceResultMessage
    ClassRegistration {
        category_id: 178,
        type_id: Uuid::from_u128(0x8c9bc99c7e1f394ab74ea12e4b9f4779),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceUseDurabilityMessage
    ClassRegistration {
        category_id: 179,
        type_id: Uuid::from_u128(0x8d429b169c2a37d59fef1b97a60a1980),
    },
    // ServiceCore.EndPointNetwork.SkillEnhanceRestoreDurabilityMessage
    ClassRegistration {
        category_id: 180,
        type_id: Uuid::from_u128(0x05ccc72801293695b7cad5978545b1b3),
    },
    // ServiceCore.EndPointNetwork.SpiritInjectionItemMessage
    ClassRegistration {
        category_id: 181,
        type_id: Uuid::from_u128(0x0e460304f4af398e86c3390a52431313),
    },
    // ServiceCore.EndPointNetwork.SpiritInjectionItemResultMessage
    ClassRegistration {
        category_id: 182,
        type_id: Uuid::from_u128(0x7cfbe0f7d7523b3c8648ac61aa980a30),
    },
    // ServiceCore.EndPointNetwork.StartGameAckMessage
    ClassRegistration {
        category_id: 183,
        type_id: Uuid::from_u128(0x019290c82e5332d4b46bf8533d59c045),
    },
    // ServiceCore.EndPointNetwork.StartTrialEventMessage
    ClassRegistration {
        category_id: 184,
        type_id: Uuid::from_u128(0x1855435aa062319c8508d07b269ee988),
    },
    // ServiceCore.EndPointNetwork.SyncFeatureMatrixMessage
    ClassRegistration {
        category_id: 185,
        type_id: Uuid::from_u128(0x966ed54f412937ada92266fa4360f61a),
    },
    // ServiceCore.EndPointNetwork.TiticoreDisplayItemsMessage
    ClassRegistration {
        category_id: 186,
        type_id: Uuid::from_u128(0x8bf1fa25574e3334819ba21423c59c6e),
    },
    // ServiceCore.EndPointNetwork.TiticoreResultMessage
    ClassRegistration {
        category_id: 187,
        type_id: Uuid::from_u128(0xd4e434d767813d458fbe345edf3786f5),
    },
    // ServiceCore.EndPointNetwork.TodayMissionInitializeMessage
    ClassRegistration {
        category_id: 188,
        type_id: Uuid::from_u128(0xa233365901373aac9be309e171335c15),
    },
    // ServiceCore.EndPointNetwork.TodayMissionUpdateMessage
    ClassRegistration {
        category_id: 189,
        type_id: Uuid::from_u128(0x066b63b05e9a3975b1c004b9a07af0ef),
    },
    // ServiceCore.EndPointNetwork.UpdateBattleInventoryInTownMessage
    ClassRegistration {
        category_id: 190,
        type_id: Uuid::from_u128(0x110880d092dd37759902d06938b010ec),
    },
    // ServiceCore.EndPointNetwork.UpdateTitleMessage
    ClassRegistration {
        category_id: 191,
        type_id: Uuid::from_u128(0xbb01bf3c81943f9494804bf20654b7b3),
    },
    // ServiceCore.EndPointNetwork.UseFeverPointMessage
    ClassRegistration {
        category_id: 192,
        type_id: Uuid::from_u128(0xb5eb2dc4303233088a7eaf1b42ba9f5f),
    },
    // ServiceCore.EndPointNetwork.UseInventoryItemWithTargetMessage
    ClassRegistration {
        category_id: 193,
        type_id: Uuid::from_u128(0x752ab1747baf3bf19ca133c315e03382),
    },
    // ServiceCore.EndPointNetwork.UserCareItemOpenMessage
    ClassRegistration {
        category_id: 194,
        type_id: Uuid::from_u128(0xce6607061b11372287477515bf7544f1),
    },
    // ServiceCore.EndPointNetwork.UserDS.UserDSHostConnectionQuery
    ClassRegistration {
        category_id: 195,
        type_id: Uuid::from_u128(0xdd29bb18eb103ac2ae28f5ee2d6737a2),
    },
    // ServiceCore.EndPointNetwork.UserDS.UserDSHostTransferStartMessage
    ClassRegistration {
        category_id: 196,
        type_id: Uuid::from_u128(0x877591d831293924857cae605dd60642),
    },
    // ServiceCore.EndPointNetwork.UserDS.UserDSLaunchMessage
    ClassRegistration {
        category_id: 197,
        type_id: Uuid::from_u128(0xb1378f498c9b3847bcdeb5eead05398c),
    },
    // ServiceCore.EndPointNetwork.UserDS.UserDSProcessEndMessage
    ClassRegistration {
        category_id: 198,
        type_id: Uuid::from_u128(0xeafd341483903daa8e8ed472756d46c9),
    },
    // ServiceCore.EndPointNetwork.UserDS.UserDSProcessStartMessage
    ClassRegistration {
        category_id: 199,
        type_id: Uuid::from_u128(0xdfe5896cc2ea351e9cb551a33e3f6487),
    },
    // ServiceCore.EndPointNetwork.UserPunishNotifyMessage
    ClassRegistration {
        category_id: 200,
        type_id: Uuid::from_u128(0xf679762910623c178f405276e1105ac4),
    },
    // ServiceCore.EndPointNetwork.Captcha.UserPunishReloadMessage
    ClassRegistration {
        category_id: 201,
        type_id: Uuid::from_u128(0x822ec65208063d6dbbdb29cc8815b14d),
    },
    // ServiceCore.EndPointNetwork.UseTiticoreItemMessage
    ClassRegistration {
        category_id: 202,
        type_id: Uuid::from_u128(0xc2ccecf7757434f8a506c295ac46cc4a),
    },
    // ServiceCore.EndPointNetwork.TownEffectMessage
    ClassRegistration {
        category_id: 203,
        type_id: Uuid::from_u128(0xc1ae394c412a3be5bcf913caa7f61535),
    },
    // ServiceCore.EndPointNetwork.VocationResetMessage
    ClassRegistration {
        category_id: 204,
        type_id: Uuid::from_u128(0x99c3179bae7939398e504676551d52bd),
    },
    // ServiceCore.EndPointNetwork.VocationResetResultMessage
    ClassRegistration {
        category_id: 205,
        type_id: Uuid::from_u128(0x2c44fa0ab3dc36839e5a3891f09ffe89),
    },
    // ServiceCore.EndPointNetwork.RequestWebVerificationMessage
    ClassRegistration {
        category_id: 206,
        type_id: Uuid::from_u128(0xe01f69b5e8d3313f99197f5281f04ca1),
    },
    // ServiceCore.EndPointNetwork.WebVerificationMessage
    ClassRegistration {
        category_id: 207,
        type_id: Uuid::from_u128(0xcef6f012f8c23f539bb67fe31338bafd),
    },
    // ServiceCore.EndPointNetwork.WishItemInfo
    ClassRegistration {
        category_id: 208,
        type_id: Uuid::from_u128(0xe15b2ea310213386a921e62728ae486d),
    },
    // ServiceCore.EndPointNetwork.WishListDeleteMessage
    ClassRegistration {
        category_id: 209,
        type_id: Uuid::from_u128(0xc4587129aa0e3a5da47d6e17ce2550a7),
    },
    // ServiceCore.EndPointNetwork.WishListDeleteResponseMessage
    ClassRegistration {
        category_id: 210,
        type_id: Uuid::from_u128(0xa72f021b568b3ff684003c6a64c3c2bc),
    },
    // ServiceCore.EndPointNetwork.WishListInsertResponseMessage
    ClassRegistration {
        category_id: 211,
        type_id: Uuid::from_u128(0xa48093cea0b93bcda52b6738c45cf30d),
    },
    // ServiceCore.EndPointNetwork.WishListInsertMessage
    ClassRegistration {
        category_id: 212,
        type_id: Uuid::from_u128(0xa522e7d8cc68346c907ab11cd6630452),
    },
    // ServiceCore.EndPointNetwork.WishListSelectMessage
    ClassRegistration {
        category_id: 213,
        type_id: Uuid::from_u128(0x97389473f9943d1daaa8df5138bdffc6),
    },
    // ServiceCore.EndPointNetwork.WishListSelectResponseMessage
    ClassRegistration {
        category_id: 214,
        type_id: Uuid::from_u128(0x643c7092008e37809e30faa5fc68b1b5),
    },
    // ServiceCore.EndPointNetwork.XignCodeReadyMessage
    ClassRegistration {
        category_id: 215,
        type_id: Uuid::from_u128(0x880f00ed086d3b12bbd12bafafb4b9bb),
    },
    // ServiceCore.EndPointNetwork.XignCodeSecureDataMessage
    ClassRegistration {
        category_id: 216,
        type_id: Uuid::from_u128(0xf8f3663bd5193a0bace27cded223663d),
    },
    // ServiceCore.EndPointNetwork._CheatJoinPartyByUserID
    ClassRegistration {
        category_id: 217,
        type_id: Uuid::from_u128(0x7dc6f921d179398fa7dc296f93191544),
    },
    // ServiceCore.EndPointNetwork.SharedInventoryInfoMessage
    ClassRegistration {
        category_id: 218,
        type_id: Uuid::from_u128(0x5c28356048793631ab337e656a881229),
    },
    // ServiceCore.EndPointNetwork.UseExtraSharedStorageMessage
    ClassRegistration {
        category_id: 219,
        type_id: Uuid::from_u128(0xabc60b1cfc2b36afb01e59bd018da347),
    },
    // ServiceCore.EndPointNetwork.UpdateSharedInventoryInfoMessage
    ClassRegistration {
        category_id: 220,
        type_id: Uuid::from_u128(0x6bfaab0927373aca933f24ba863eb88b),
    },
    // ServiceCore.EndPointNetwork.UpdateSharedStorageInfoMessage
    ClassRegistration {
        category_id: 221,
        type_id: Uuid::from_u128(0xac65fcedb2ae30f5a3bb17b71a7b7186),
    },
    // ServiceCore.EndPointNetwork.RejoinCombatSuccessMessage
    ClassRegistration {
        category_id: 222,
        type_id: Uuid::from_u128(0x130389e1978a3ddd984f089fa468f1f5),
    },
    // ServiceCore.EndPointNetwork.AcceptAssistMessage
    ClassRegistration {
        category_id: 223,
        type_id: Uuid::from_u128(0x79aa3c3c437d3e0abc5887f7ce1aacec),
    },
    // ServiceCore.EndPointNetwork.AcceptQuestMessage
    ClassRegistration {
        category_id: 224,
        type_id: Uuid::from_u128(0xaecbcd0a82b331eeb771fe9c53dabdd8),
    },
    // ServiceCore.EndPointNetwork.AdditionalMicroPlayContentsMessage
    ClassRegistration {
        category_id: 225,
        type_id: Uuid::from_u128(0x26c74c1533c031edaea699770e06c127),
    },
    // ServiceCore.EndPointNetwork.DestroyMicroPlayContentsMessage
    ClassRegistration {
        category_id: 226,
        type_id: Uuid::from_u128(0xfca127605c0937039e5848e4b989e1b2),
    },
    // ServiceCore.EndPointNetwork.SpawnMonsterMessage
    ClassRegistration {
        category_id: 227,
        type_id: Uuid::from_u128(0xd19bb88d885037729b6b591993559dec),
    },
    // ServiceCore.EndPointNetwork.AddStatusEffect
    ClassRegistration {
        category_id: 228,
        type_id: Uuid::from_u128(0x010813f1e0aa30078497f1086be09922),
    },
    // ServiceCore.EndPointNetwork.APMessage
    ClassRegistration {
        category_id: 229,
        type_id: Uuid::from_u128(0x3d59ac686b3c3f92968eb569b973f065),
    },
    // ServiceCore.EndPointNetwork.ArmorBrokenMessage
    ClassRegistration {
        category_id: 230,
        type_id: Uuid::from_u128(0x2d0a5f45031437bdaf4f4c68545880d8),
    },
    // ServiceCore.EndPointNetwork.AskSecondPasswordMessage
    ClassRegistration {
        category_id: 231,
        type_id: Uuid::from_u128(0xe9e7d6a121313b59826d88c968ae0867),
    },
    // ServiceCore.EndPointNetwork.AssignMissionSuccessMessage
    ClassRegistration {
        category_id: 232,
        type_id: Uuid::from_u128(0x8494d07db5ae3e5b9cb51884ca82047b),
    },
    // ServiceCore.EndPointNetwork.BeautyShopCustomizeMessage
    ClassRegistration {
        category_id: 233,
        type_id: Uuid::from_u128(0x31e4e1d4592f3f80b09427cc6a284f97),
    },
    // ServiceCore.EndPointNetwork.BuyItemMessage
    ClassRegistration {
        category_id: 234,
        type_id: Uuid::from_u128(0x2f52e15133c134dcbd6e9535c35ab918),
    },
    // ServiceCore.EndPointNetwork.BuyItemResultMessage
    ClassRegistration {
        category_id: 235,
        type_id: Uuid::from_u128(0x64a08bdf67623be29f6fc7ff64a2b4a3),
    },
    // ServiceCore.EndPointNetwork.CafeLoginMessage
    ClassRegistration {
        category_id: 236,
        type_id: Uuid::from_u128(0x3c438b534b8739819ef1e164d6632266),
    },
    // ServiceCore.EndPointNetwork.CashShopGiftSenderMessage
    ClassRegistration {
        category_id: 237,
        type_id: Uuid::from_u128(0xa0d1bae01b9d3bddbd0dc5072ae07028),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopBalanceMessage
    ClassRegistration {
        category_id: 238,
        type_id: Uuid::from_u128(0x1979a6adf97532689ebf19875d68756b),
    },
    // ServiceCore.EndPointNetwork.CashShopBalanceMessage
    ClassRegistration {
        category_id: 239,
        type_id: Uuid::from_u128(0xb5b3994043ec33ecaa7b3f786d4bd5ac),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopInventoryMessage
    ClassRegistration {
        category_id: 240,
        type_id: Uuid::from_u128(0x126717966f2131488097e5f33dd30899),
    },
    // ServiceCore.EndPointNetwork.CashShopInventoryMessage
    ClassRegistration {
        category_id: 241,
        type_id: Uuid::from_u128(0xc99ca6d79067380e9b93b9c3cfeab051),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopProductListMessage
    ClassRegistration {
        category_id: 242,
        type_id: Uuid::from_u128(0xc60512f1a1973109a28bf89c6003d9fb),
    },
    // ServiceCore.EndPointNetwork.CashShopProductListMessage
    ClassRegistration {
        category_id: 243,
        type_id: Uuid::from_u128(0x43148f519ebf3fb3a2ffceb7fe3503f2),
    },
    // ServiceCore.EndPointNetwork.CashShopCategoryListMessage
    ClassRegistration {
        category_id: 244,
        type_id: Uuid::from_u128(0xd5d8393b80ea36fba7240dc9e66c1b32),
    },
    // ServiceCore.EndPointNetwork.QueryBeautyShopInfoMessage
    ClassRegistration {
        category_id: 245,
        type_id: Uuid::from_u128(0x76f3a59fa67939a1bf25011a12b67fe1),
    },
    // ServiceCore.EndPointNetwork.BeautyShopInfoMessage
    ClassRegistration {
        category_id: 246,
        type_id: Uuid::from_u128(0xc7bc21c4336d3d64aec5043b918f32ce),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopItemPickUpMessage
    ClassRegistration {
        category_id: 247,
        type_id: Uuid::from_u128(0xea22129f5ff53a25934c764318735384),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopPurchaseItemMessage
    ClassRegistration {
        category_id: 248,
        type_id: Uuid::from_u128(0x52d58673c69c39769e4bb57e2d9adc9e),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopPurchaseGiftMessage
    ClassRegistration {
        category_id: 249,
        type_id: Uuid::from_u128(0x41a793d65a513dbcb3e7296310fc558a),
    },
    // ServiceCore.EndPointNetwork.CashShopFailMessage
    ClassRegistration {
        category_id: 250,
        type_id: Uuid::from_u128(0x6a2570387e653370886dbb0c30c7e1c8),
    },
    // ServiceCore.EndPointNetwork.RequestGoddessProtectionMessage
    ClassRegistration {
        category_id: 251,
        type_id: Uuid::from_u128(0x7d0484cf10a630abb2814572db100ec0),
    },
    // ServiceCore.EndPointNetwork.DirectPurchaseCashShopItemMessage
    ClassRegistration {
        category_id: 252,
        type_id: Uuid::from_u128(0xd45087124d42395d9e837b168b79f55d),
    },
    // ServiceCore.EndPointNetwork.DirectPurchaseCashShopItemResultMessage
    ClassRegistration {
        category_id: 253,
        type_id: Uuid::from_u128(0x0460ce89d423379f9dbb970bda070dd0),
    },
    // ServiceCore.EndPointNetwork.DirectPurchaseTiticoreItemMessage
    ClassRegistration {
        category_id: 254,
        type_id: Uuid::from_u128(0xc879c0b23250353b86d077792c7ec905),
    },
    // ServiceCore.EndPointNetwork.DirectPurchaseTiticoreItemResultMessage
    ClassRegistration {
        category_id: 255,
        type_id: Uuid::from_u128(0x8d3522ab2a99396bb11e14f2935f3576),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopRefundMessage
    ClassRegistration {
        category_id: 256,
        type_id: Uuid::from_u128(0xd64a50e94cfa3cd4b710beecc66526c2),
    },
    // ServiceCore.EndPointNetwork.ChangeMissionStatusMessage
    ClassRegistration {
        category_id: 257,
        type_id: Uuid::from_u128(0xf89a33b6e5f338179b4746f833ab038f),
    },
    // ServiceCore.EndPointNetwork.CharacterList.DeleteCharacterCancelMessage
    ClassRegistration {
        category_id: 258,
        type_id: Uuid::from_u128(0x7a123c939f063bba8e0e1bf3d7a1d512),
    },
    // ServiceCore.EndPointNetwork.CharacterList.DeleteCharacterCancelResultMessage
    ClassRegistration {
        category_id: 259,
        type_id: Uuid::from_u128(0x8f6e7d118bdc3e5b970444e95b136338),
    },
    // ServiceCore.EndPointNetwork.ChatReportMessage
    ClassRegistration {
        category_id: 260,
        type_id: Uuid::from_u128(0x39b377ca968c3d01896a1e6116cede96),
    },
    // ServiceCore.EndPointNetwork.CheckCharacterNameMessage
    ClassRegistration {
        category_id: 261,
        type_id: Uuid::from_u128(0xc63a6f7537c935f68cffd7ae87ac2967),
    },
    // ServiceCore.EndPointNetwork.CompletedMissionInfoMessage
    ClassRegistration {
        category_id: 262,
        type_id: Uuid::from_u128(0x456192b9264d3b91899526ca2620fc7d),
    },
    // ServiceCore.EndPointNetwork.CombatRecordMessage
    ClassRegistration {
        category_id: 263,
        type_id: Uuid::from_u128(0x1c979684101a325cbf59caca716604fb),
    },
    // ServiceCore.EndPointNetwork.DS.DSCommandMessage
    ClassRegistration {
        category_id: 264,
        type_id: Uuid::from_u128(0x954292186a0634b486d0702d68fa0c40),
    },
    // ServiceCore.EndPointNetwork.DS.DSPlayerStatusMessage
    ClassRegistration {
        category_id: 265,
        type_id: Uuid::from_u128(0x876218424add393387ea4183916f5996),
    },
    // ServiceCore.EndPointNetwork.DS.RegisterDSQueueMessage
    ClassRegistration {
        category_id: 266,
        type_id: Uuid::from_u128(0xef831c7003d13eebbf3caeea4492f065),
    },
    // ServiceCore.EndPointNetwork.DS.UnregisterDSQueueMessage
    ClassRegistration {
        category_id: 267,
        type_id: Uuid::from_u128(0x7d3cf615fddd364e81d52b538cf9cd8f),
    },
    // ServiceCore.EndPointNetwork.DyeAmpleCompleteMessage
    ClassRegistration {
        category_id: 268,
        type_id: Uuid::from_u128(0xc976502310d13a0abf976b45e7312258),
    },
    // ServiceCore.EndPointNetwork.DyeAmpleRequestMessage
    ClassRegistration {
        category_id: 269,
        type_id: Uuid::from_u128(0xc55d23ca6885315593755035631f3ff5),
    },
    // ServiceCore.EndPointNetwork.DyeAmpleUsedMessage
    ClassRegistration {
        category_id: 270,
        type_id: Uuid::from_u128(0x25fbb3e8bab43548a0a1224aa97b4f25),
    },
    // ServiceCore.EndPointNetwork.DyeItemCashMessage
    ClassRegistration {
        category_id: 271,
        type_id: Uuid::from_u128(0xe2326d3866ce366fb06e01b39947652c),
    },
    // ServiceCore.EndPointNetwork.EffectTelepathyMessage
    ClassRegistration {
        category_id: 272,
        type_id: Uuid::from_u128(0x54b3a958d949351f860b088f11168a2d),
    },
    // ServiceCore.EndPointNetwork.EnterRegion
    ClassRegistration {
        category_id: 273,
        type_id: Uuid::from_u128(0xe579d9a554c13acebe08181fbd1d9d02),
    },
    // ServiceCore.EndPointNetwork.ExpandExpirationDateItemMessage
    ClassRegistration {
        category_id: 274,
        type_id: Uuid::from_u128(0x787049c4885b31c7a795742a27d17fbd),
    },
    // ServiceCore.EndPointNetwork.ExpandExpirationDateResponseMessage
    ClassRegistration {
        category_id: 275,
        type_id: Uuid::from_u128(0x0d473df6a6c5346abaf39aec2dade3a9),
    },
    // ServiceCore.EndPointNetwork.FreeMatchReportMessage
    ClassRegistration {
        category_id: 276,
        type_id: Uuid::from_u128(0x8673db49f849349bacea3794efe7ece5),
    },
    // ServiceCore.EndPointNetwork.GuildService.CloseGuildMessage
    ClassRegistration {
        category_id: 277,
        type_id: Uuid::from_u128(0x89f37dba2fc13b7d9512465648c48278),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildInfoMessage
    ClassRegistration {
        category_id: 278,
        type_id: Uuid::from_u128(0x647a5f98b588328fb58dddc8164d3014),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildListMessage
    ClassRegistration {
        category_id: 279,
        type_id: Uuid::from_u128(0xd74f17bb616b3a80a17f46217557ad03),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildNoticeEditMessage
    ClassRegistration {
        category_id: 280,
        type_id: Uuid::from_u128(0x3df3921046213c4ab85aa1c864c4f86d),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildResultMessage
    ClassRegistration {
        category_id: 281,
        type_id: Uuid::from_u128(0x4d108cd782733e5bafcc2d077d73e7e0),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildStoryRequestMessage
    ClassRegistration {
        category_id: 282,
        type_id: Uuid::from_u128(0xefc67527b0af310fa8b7b39c1922e670),
    },
    // ServiceCore.EndPointNetwork.GuildService.InviteGuildMessage
    ClassRegistration {
        category_id: 283,
        type_id: Uuid::from_u128(0x51b656717ce13ca785721b02fb80076a),
    },
    // ServiceCore.EndPointNetwork.GuildService.JoinGuildMessage
    ClassRegistration {
        category_id: 284,
        type_id: Uuid::from_u128(0x7abea10d56323388bea1c342d53465f8),
    },
    // ServiceCore.EndPointNetwork.GuildService.LeaveGuildMessage
    ClassRegistration {
        category_id: 285,
        type_id: Uuid::from_u128(0xeb9a9076bd3a335887b914a00200e1f8),
    },
    // ServiceCore.EndPointNetwork.GuildService.ReconnectGuildMessage
    ClassRegistration {
        category_id: 286,
        type_id: Uuid::from_u128(0x918435ec32133b76b1b8569b1f7657cb),
    },
    // ServiceCore.EndPointNetwork.UseMegaphoneMessage
    ClassRegistration {
        category_id: 287,
        type_id: Uuid::from_u128(0xc1be41d09bbf3ac5b98037804d44ba61),
    },
    // ServiceCore.EndPointNetwork.MegaphoneMessage
    ClassRegistration {
        category_id: 288,
        type_id: Uuid::from_u128(0x89369cd83ddc34ee8be6af0e15d8e007),
    },
    // ServiceCore.EndPointNetwork.MegaphoneFailMessage
    ClassRegistration {
        category_id: 289,
        type_id: Uuid::from_u128(0x5d44d34753c63a078a2b47af8fc32bc5),
    },
    // ServiceCore.EndPointNetwork.MonsterDamageReportMessage
    ClassRegistration {
        category_id: 290,
        type_id: Uuid::from_u128(0x62bd766f989936d4a6fc0e4e448471c2),
    },
    // ServiceCore.EndPointNetwork.NotifyPlayerReconnectMessage
    ClassRegistration {
        category_id: 291,
        type_id: Uuid::from_u128(0x42978bb6a445395fab3cf8e5bc4801ef),
    },
    // ServiceCore.EndPointNetwork.AnswerPlayerReconnectMessage
    ClassRegistration {
        category_id: 292,
        type_id: Uuid::from_u128(0xae461c733b3e3fc5860d2e24af209646),
    },
    // ServiceCore.EndPointNetwork.QueryBuybackListMessage
    ClassRegistration {
        category_id: 293,
        type_id: Uuid::from_u128(0x2271100aeffc3693b75d165507d49b79),
    },
    // ServiceCore.EndPointNetwork.BuybackListResultMessage
    ClassRegistration {
        category_id: 294,
        type_id: Uuid::from_u128(0x88ce410bfcb73794ab5f9cb25771ce3d),
    },
    // ServiceCore.EndPointNetwork.QuestTargetResultMessage
    ClassRegistration {
        category_id: 295,
        type_id: Uuid::from_u128(0xbfed3fca9934374fb39dcebb0729e57f),
    },
    // ServiceCore.EndPointNetwork.ResetSkillMessage
    ClassRegistration {
        category_id: 296,
        type_id: Uuid::from_u128(0x67c78207a9b1395993665decdedc8d13),
    },
    // ServiceCore.EndPointNetwork.ResetSkillResultMessage
    ClassRegistration {
        category_id: 297,
        type_id: Uuid::from_u128(0x40dbb42c72f33e7e9d9555a82f3e1c5a),
    },
    // ServiceCore.EndPointNetwork.RequestSecuredOperationMessage
    ClassRegistration {
        category_id: 298,
        type_id: Uuid::from_u128(0xd7b1541806ce3d458e4208179db5f5a6),
    },
    // ServiceCore.EndPointNetwork.SecuredOperationMessage
    ClassRegistration {
        category_id: 299,
        type_id: Uuid::from_u128(0xe4c5c60688ae304e943197df6b3c4419),
    },
    // ServiceCore.EndPointNetwork.TcProtectRequestMessage
    ClassRegistration {
        category_id: 300,
        type_id: Uuid::from_u128(0x53cc25f70f533edfa81347969888d5c0),
    },
    // ServiceCore.EndPointNetwork.TcProtectRespondMessage
    ClassRegistration {
        category_id: 301,
        type_id: Uuid::from_u128(0x4d59f3fb688832f0848f3e2644378930),
    },
    // ServiceCore.EndPointNetwork.TradeCancelMyItemMessage
    ClassRegistration {
        category_id: 302,
        type_id: Uuid::from_u128(0x63d594016b18305aaf2f98a77d2d977b),
    },
    // ServiceCore.EndPointNetwork.TradeCancelMyItemResultMessage
    ClassRegistration {
        category_id: 303,
        type_id: Uuid::from_u128(0xbbae86b329cf3b81afd55736213f0c2d),
    },
    // ServiceCore.EndPointNetwork.DetailOption
    ClassRegistration {
        category_id: 304,
        type_id: Uuid::from_u128(0x326b6bd4c546318b8179891a7f794c10),
    },
    // ServiceCore.EndPointNetwork.TradeCategorySearchMessage
    ClassRegistration {
        category_id: 305,
        type_id: Uuid::from_u128(0x77610fcffc8736338f8ede283eff2e6f),
    },
    // ServiceCore.EndPointNetwork.TradeItemClassListSearchMessage
    ClassRegistration {
        category_id: 306,
        type_id: Uuid::from_u128(0x92300b56a6fc35ecaa0cc344e2615e6f),
    },
    // ServiceCore.EndPointNetwork.TradeItemInfo
    ClassRegistration {
        category_id: 307,
        type_id: Uuid::from_u128(0x8b62b236a7f43f0d9aec40cacb0997d0),
    },
    // ServiceCore.EndPointNetwork.TradeMyItemInfos
    ClassRegistration {
        category_id: 308,
        type_id: Uuid::from_u128(0x1d431b502a5d3e6e829f8c1a5057b5b3),
    },
    // ServiceCore.EndPointNetwork.TradePurchaseItemMessage
    ClassRegistration {
        category_id: 309,
        type_id: Uuid::from_u128(0x6d284225067630019369ba59fe31cb4f),
    },
    // ServiceCore.EndPointNetwork.TradePurchaseItemResultMessage
    ClassRegistration {
        category_id: 310,
        type_id: Uuid::from_u128(0x813c7eafa144334e81cd9d066f11000e),
    },
    // ServiceCore.EndPointNetwork.TradeRequestMyItemMessage
    ClassRegistration {
        category_id: 311,
        type_id: Uuid::from_u128(0xe0188571a1a33b9387729c44ad8d1114),
    },
    // ServiceCore.EndPointNetwork.TradeSearchResult
    ClassRegistration {
        category_id: 312,
        type_id: Uuid::from_u128(0xf6cbae04367a3926b010aaed1b28790a),
    },
    // ServiceCore.EndPointNetwork.MovePetSlotMessage
    ClassRegistration {
        category_id: 313,
        type_id: Uuid::from_u128(0x86698206912c36be9cc1257a396935fb),
    },
    // ServiceCore.EndPointNetwork.RemovePetMessage
    ClassRegistration {
        category_id: 314,
        type_id: Uuid::from_u128(0x9f8b0d6dae8e388f884111aa2aa37a0d),
    },
    // ServiceCore.EndPointNetwork.SelectPetMessage
    ClassRegistration {
        category_id: 315,
        type_id: Uuid::from_u128(0x5736fde9bc213b82a84e72bb0a517a14),
    },
    // ServiceCore.EndPointNetwork.QueryPetListMessage
    ClassRegistration {
        category_id: 316,
        type_id: Uuid::from_u128(0xe94af65de66f31fdb8d053a042370b5c),
    },
    // ServiceCore.EndPointNetwork.PetRevivedMessage
    ClassRegistration {
        category_id: 317,
        type_id: Uuid::from_u128(0xe145a882d89c349497bf607b1aeaaab5),
    },
    // ServiceCore.EndPointNetwork.PetKilledMessage
    ClassRegistration {
        category_id: 318,
        type_id: Uuid::from_u128(0x3079bc3a9b673de0a50487a66664647f),
    },
    // ServiceCore.EndPointNetwork.RemovePetSkillMessage
    ClassRegistration {
        category_id: 319,
        type_id: Uuid::from_u128(0x8aaf9011eb3a3fc1a40b25602676547b),
    },
    // ServiceCore.EndPointNetwork.PetCreateNameCheckMessage
    ClassRegistration {
        category_id: 320,
        type_id: Uuid::from_u128(0xefd95e5704fa3e9392719bcc29422fb0),
    },
    // ServiceCore.EndPointNetwork.PetCreateNameCheckResultMessage
    ClassRegistration {
        category_id: 321,
        type_id: Uuid::from_u128(0x133b280b47e434ccb16aa8a03eb4dc97),
    },
    // ServiceCore.EndPointNetwork.PetCreateMessage
    ClassRegistration {
        category_id: 322,
        type_id: Uuid::from_u128(0x942945c3e95f3bb49be2a7f984434f41),
    },
    // ServiceCore.EndPointNetwork.UpdateCurrentPetMessage
    ClassRegistration {
        category_id: 323,
        type_id: Uuid::from_u128(0x9fddbc1b3c9e3c798b50457c8379c699),
    },
    // ServiceCore.EndPointNetwork.PetListMessage
    ClassRegistration {
        category_id: 324,
        type_id: Uuid::from_u128(0x62804a1d30eb310c87920b6aa839ffd3),
    },
    // ServiceCore.EndPointNetwork.PetFeedListMessage
    ClassRegistration {
        category_id: 325,
        type_id: Uuid::from_u128(0x2fcaaf5ca077375d933c92d298581487),
    },
    // ServiceCore.EndPointNetwork.RequestPetFoodShareMessage
    ClassRegistration {
        category_id: 326,
        type_id: Uuid::from_u128(0x0d488183a8d0373099ab1088836ab2d4),
    },
    // ServiceCore.EndPointNetwork.RequestPetFoodUnshareMessage
    ClassRegistration {
        category_id: 327,
        type_id: Uuid::from_u128(0x5922ce58625f3c918b087331f63b5a27),
    },
    // ServiceCore.EndPointNetwork.PetChangeNameCheckMessage
    ClassRegistration {
        category_id: 328,
        type_id: Uuid::from_u128(0xad77125f4472336fa50e45d79ec35a0e),
    },
    // ServiceCore.EndPointNetwork.PetChangeNameCheckResultMessage
    ClassRegistration {
        category_id: 329,
        type_id: Uuid::from_u128(0x8b057824a0313410a24bf52be6c03cf3),
    },
    // ServiceCore.EndPointNetwork.PetChangeNameMessage
    ClassRegistration {
        category_id: 330,
        type_id: Uuid::from_u128(0x041d653f01e7371ab86a1346b110d36c),
    },
    // ServiceCore.EndPointNetwork.PetChangeNameResultMessage
    ClassRegistration {
        category_id: 331,
        type_id: Uuid::from_u128(0xa9c12ba227cd3864978642d4f295dff5),
    },
    // ServiceCore.EndPointNetwork.PetOperationMessage
    ClassRegistration {
        category_id: 332,
        type_id: Uuid::from_u128(0xbf09247937753d898f7753bcc8c681f4),
    },
    // ServiceCore.EndPointNetwork.QueryRankAlarmInfoMessage
    ClassRegistration {
        category_id: 333,
        type_id: Uuid::from_u128(0x81ccd497b4c135d7b08c77ddf2073089),
    },
    // ServiceCore.EndPointNetwork.RankAlarmInfoMessage
    ClassRegistration {
        category_id: 334,
        type_id: Uuid::from_u128(0xb1f6d4f1e60730fa915f1e5e1c14ed0c),
    },
    // ServiceCore.EndPointNetwork.TransferPartyMasterMessage
    ClassRegistration {
        category_id: 335,
        type_id: Uuid::from_u128(0x149b048b3373373bbbc26304a6bad3ae),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildMemberListMessage
    ClassRegistration {
        category_id: 336,
        type_id: Uuid::from_u128(0xe9a6338219a439b9915a765a485a84b0),
    },
    // ServiceCore.EndPointNetwork.GuildService.GuildOperationMessage
    ClassRegistration {
        category_id: 337,
        type_id: Uuid::from_u128(0x0c77318bcd6a3bd68d6c1e26f68f8717),
    },
    // ServiceCore.EndPointNetwork.GuildService.OpenGuildMessage
    ClassRegistration {
        category_id: 338,
        type_id: Uuid::from_u128(0x37c909f5c8f633d28f2c7dda5b1fe08d),
    },
    // ServiceCore.EndPointNetwork.GuildService.QueryGuildListMessage
    ClassRegistration {
        category_id: 339,
        type_id: Uuid::from_u128(0x34da3a6de9a23f2f83a6e73965a7b636),
    },
    // ServiceCore.EndPointNetwork.LearnNewSkillMessage
    ClassRegistration {
        category_id: 340,
        type_id: Uuid::from_u128(0x7fbd47e196a334729e71f36d90f7294f),
    },
    // ServiceCore.EndPointNetwork.LearnNewSkillResultMessage
    ClassRegistration {
        category_id: 341,
        type_id: Uuid::from_u128(0x0430910fe20e3736850db619e836f14f),
    },
    // ServiceCore.EndPointNetwork.MaxDurabilityRepairItemMessage
    ClassRegistration {
        category_id: 342,
        type_id: Uuid::from_u128(0x66ecfccac35b3005a79cad9fc04da573),
    },
    // ServiceCore.EndPointNetwork.GetCharacterMissionStatusMessage
    ClassRegistration {
        category_id: 343,
        type_id: Uuid::from_u128(0xace7d955f22539a58b56ad747ded5e3d),
    },
    // ServiceCore.EndPointNetwork.MissionMessage
    ClassRegistration {
        category_id: 344,
        type_id: Uuid::from_u128(0xcc00a15bc2a2348bbaa877661dc82ab4),
    },
    // ServiceCore.EndPointNetwork.NoticeGameEnvironmentMessage
    ClassRegistration {
        category_id: 345,
        type_id: Uuid::from_u128(0x97636de86ff13d4c8c53b8f1da851df4),
    },
    // ServiceCore.EndPointNetwork.OpenPartyWithShipInfoMessage
    ClassRegistration {
        category_id: 346,
        type_id: Uuid::from_u128(0xcf6e4d7d1568379a80e4cd169312a484),
    },
    // ServiceCore.EndPointNetwork.PostingInfoMessage
    ClassRegistration {
        category_id: 347,
        type_id: Uuid::from_u128(0x38fe11d26dc63bbd8b1b4be8f5575f92),
    },
    // ServiceCore.EndPointNetwork.RandomMissionMessage
    ClassRegistration {
        category_id: 348,
        type_id: Uuid::from_u128(0x230181995e5a34039294088b9f51856b),
    },
    // ServiceCore.EndPointNetwork.ClientLogMessage
    ClassRegistration {
        category_id: 349,
        type_id: Uuid::from_u128(0x7a8ecc83fea130b4ad37b47f0563fce1),
    },
    // ServiceCore.EndPointNetwork.DropGoldCoreMessage
    ClassRegistration {
        category_id: 350,
        type_id: Uuid::from_u128(0xd01fb1ee5a733dee834a84687c126f14),
    },
    // ServiceCore.EndPointNetwork.RarityCoreListMessage
    ClassRegistration {
        category_id: 351,
        type_id: Uuid::from_u128(0x74a38d1c639b36459086177249d79520),
    },
    // ServiceCore.EndPointNetwork.EnchantItemMessage
    ClassRegistration {
        category_id: 352,
        type_id: Uuid::from_u128(0xddc9fc48e5dc304ebfd28c5fed04f83e),
    },
    // ServiceCore.EndPointNetwork.EnchantItemResultMessage
    ClassRegistration {
        category_id: 353,
        type_id: Uuid::from_u128(0x0a5b1d797e6332d79520ab7d8bfaa7ed),
    },
    // ServiceCore.EndPointNetwork.EnhanceItemMessage
    ClassRegistration {
        category_id: 354,
        type_id: Uuid::from_u128(0x04804f385cec31f6885acc0fc33628eb),
    },
    // ServiceCore.EndPointNetwork.EnhanceItemResultMessage
    ClassRegistration {
        category_id: 355,
        type_id: Uuid::from_u128(0x1726cb8834093a2b892356e0250aab6a),
    },
    // ServiceCore.EndPointNetwork.HostRestartedMessage
    ClassRegistration {
        category_id: 356,
        type_id: Uuid::from_u128(0xac095fc86eb23c669f5bb3c788d6b24a),
    },
    // ServiceCore.EndPointNetwork.HostRestartingMessage
    ClassRegistration {
        category_id: 357,
        type_id: Uuid::from_u128(0x9d9b2fd5fb1d3e2e9bdd9c7f85a308ec),
    },
    // ServiceCore.EndPointNetwork.ItemFailMessage
    ClassRegistration {
        category_id: 358,
        type_id: Uuid::from_u128(0x2f756b908fba38b8bcb2a51f0f132502),
    },
    // ServiceCore.EndPointNetwork.ManufactureInfoMessage
    ClassRegistration {
        category_id: 359,
        type_id: Uuid::from_u128(0x9848342dcc2f326e9b4ed30c6decd5b5),
    },
    // ServiceCore.EndPointNetwork.ManufactureCraftMessage
    ClassRegistration {
        category_id: 360,
        type_id: Uuid::from_u128(0x42f64ed3c84b3cfb853b0dec85cfac07),
    },
    // ServiceCore.EndPointNetwork.ManufactureLevelUpMessage
    ClassRegistration {
        category_id: 361,
        type_id: Uuid::from_u128(0x4b8c26ec64123016b31e157b18638117),
    },
    // ServiceCore.EndPointNetwork.MicroPlayEventMessage
    ClassRegistration {
        category_id: 362,
        type_id: Uuid::from_u128(0x52a3dbeb8337328890055557bfa8e7e0),
    },
    // ServiceCore.EndPointNetwork.Identify
    ClassRegistration {
        category_id: 363,
        type_id: Uuid::from_u128(0x428dee64190d3cf684503e511769c608),
    },
    // ServiceCore.EndPointNetwork.EnterChannel
    ClassRegistration {
        category_id: 364,
        type_id: Uuid::from_u128(0x79ce9a5389563645a67bf69681ea7a35),
    },
    // ServiceCore.EndPointNetwork.ChannelServerAddress
    ClassRegistration {
        category_id: 365,
        type_id: Uuid::from_u128(0x85bf1995487c3e7e8603e88d75d21b46),
    },
    // ServiceCore.EndPointNetwork.IdentifyFailed
    ClassRegistration {
        category_id: 366,
        type_id: Uuid::from_u128(0x42fa5d33391e320b85db0c61e1b7f6ba),
    },
    // ServiceCore.EndPointNetwork.ChannelChanged
    ClassRegistration {
        category_id: 367,
        type_id: Uuid::from_u128(0xb10fb659d06231a09b3318e4cee64330),
    },
    // ServiceCore.EndPointNetwork.MovePartition
    ClassRegistration {
        category_id: 368,
        type_id: Uuid::from_u128(0xd3cf4e65f3a637f4aae6d8819a02e728),
    },
    // ServiceCore.EndPointNetwork.UpdateAction
    ClassRegistration {
        category_id: 369,
        type_id: Uuid::from_u128(0xbb98e908edbe32d589c2eb38d6508cad),
    },
    // ServiceCore.EndPointNetwork.Chat
    ClassRegistration {
        category_id: 370,
        type_id: Uuid::from_u128(0x7b77403748dc3a899bfc4467a1978c43),
    },
    // ServiceCore.EndPointNetwork.Leave
    ClassRegistration {
        category_id: 371,
        type_id: Uuid::from_u128(0xf2341a29eb0d3192add2ffa5bef614b1),
    },
    // ServiceCore.EndPointNetwork.Disappeared
    ClassRegistration {
        category_id: 372,
        type_id: Uuid::from_u128(0x4f13a7053f0237cbb869ebfbf3852ce2),
    },
    // ServiceCore.EndPointNetwork.NotifyLook
    ClassRegistration {
        category_id: 373,
        type_id: Uuid::from_u128(0x711f48aec601344cbb5a9320b001f809),
    },
    // ServiceCore.EndPointNetwork.NotifyAction
    ClassRegistration {
        category_id: 374,
        type_id: Uuid::from_u128(0x86b97474ac743722bda1a8ba43968733),
    },
    // ServiceCore.EndPointNetwork.NotifyChat
    ClassRegistration {
        category_id: 375,
        type_id: Uuid::from_u128(0xdc46777e8e6a37faac811247b40b35e4),
    },
    // ServiceCore.EndPointNetwork.PvpCommandMessage
    ClassRegistration {
        category_id: 376,
        type_id: Uuid::from_u128(0xb27a17ad9e01361b933e701ccaec625d),
    },
    // ServiceCore.EndPointNetwork.PvpKickedMessage
    ClassRegistration {
        category_id: 377,
        type_id: Uuid::from_u128(0x542086a0b1ac3f388f87ea16e3b162c7),
    },
    // ServiceCore.EndPointNetwork.PvpRegisterResultMessage
    ClassRegistration {
        category_id: 378,
        type_id: Uuid::from_u128(0xc4cee3b548bb3b17a6b70b05e7c4e803),
    },
    // ServiceCore.EndPointNetwork.PvpReportMessage
    ClassRegistration {
        category_id: 379,
        type_id: Uuid::from_u128(0x8e68a8d99e063199a19a9e9cdf506748),
    },
    // ServiceCore.EndPointNetwork.PvpTeamInfoMessage
    ClassRegistration {
        category_id: 380,
        type_id: Uuid::from_u128(0xf851a062d98a314da92b060567a6b060),
    },
    // ServiceCore.EndPointNetwork.RandomItemRewardListMessage
    ClassRegistration {
        category_id: 381,
        type_id: Uuid::from_u128(0x4ee2fd65d4643707b8763280b7d88f04),
    },
    // ServiceCore.EndPointNetwork.QueryRankAllInfoMessage
    ClassRegistration {
        category_id: 382,
        type_id: Uuid::from_u128(0x76e547c77cf9352396be0d5367e969aa),
    },
    // ServiceCore.EndPointNetwork.QueryRankCharacterInfoMessage
    ClassRegistration {
        category_id: 383,
        type_id: Uuid::from_u128(0xd2180bc706fd38e395e97801bcb01860),
    },
    // ServiceCore.EndPointNetwork.QueryRankFavoritesInfoMessage
    ClassRegistration {
        category_id: 384,
        type_id: Uuid::from_u128(0x22ab80f4a54c3a028a37daad929429a9),
    },
    // ServiceCore.EndPointNetwork.QueryRankGoalInfoMessage
    ClassRegistration {
        category_id: 385,
        type_id: Uuid::from_u128(0x05c1e7bd644831a697982d9c2cbec28e),
    },
    // ServiceCore.EndPointNetwork.QueryRankHeavyFavoritesInfoMessage
    ClassRegistration {
        category_id: 386,
        type_id: Uuid::from_u128(0xecdf4e69d15e37a0a873534ca32e79dd),
    },
    // ServiceCore.EndPointNetwork.QueryRankInfoMessage
    ClassRegistration {
        category_id: 387,
        type_id: Uuid::from_u128(0x9a9b066c03e83ebfa2abe171a2b6665d),
    },
    // ServiceCore.EndPointNetwork.QueryRankOtherCharacterInfoMessage
    ClassRegistration {
        category_id: 388,
        type_id: Uuid::from_u128(0x46da8595f780315786e8ed68f230bd60),
    },
    // ServiceCore.EndPointNetwork.RankCharacterInfoMessage
    ClassRegistration {
        category_id: 389,
        type_id: Uuid::from_u128(0x069fe707679c3fada66ff2cae935c923),
    },
    // ServiceCore.EndPointNetwork.RankFavoritesInfoMessage
    ClassRegistration {
        category_id: 390,
        type_id: Uuid::from_u128(0xbb38aa48cb2a3db0a98a862a571abda7),
    },
    // ServiceCore.EndPointNetwork.RankGoalInfoMessage
    ClassRegistration {
        category_id: 391,
        type_id: Uuid::from_u128(0x5652a3976ea33b229c377c6969a8902f),
    },
    // ServiceCore.EndPointNetwork.RankHeavyFavoritesInfoMessage
    ClassRegistration {
        category_id: 392,
        type_id: Uuid::from_u128(0xff2edfd744a3356d9f056dcfbd9447a1),
    },
    // ServiceCore.EndPointNetwork.RankAllInfoMessage
    ClassRegistration {
        category_id: 393,
        type_id: Uuid::from_u128(0x61439335cfa235b99d41d26c4778dd90),
    },
    // ServiceCore.EndPointNetwork.RankInfoMessage
    ClassRegistration {
        category_id: 394,
        type_id: Uuid::from_u128(0x7d4ee0f1991f3ca7ae3d2fd9ec4be411),
    },
    // ServiceCore.EndPointNetwork.RankOtherCharacterInfoMessage
    ClassRegistration {
        category_id: 395,
        type_id: Uuid::from_u128(0x70712a6ca1c3330abb24c8bb9d0b6830),
    },
    // ServiceCore.EndPointNetwork.RequestSetRankFavoriteInfoMessage
    ClassRegistration {
        category_id: 396,
        type_id: Uuid::from_u128(0x8824e598669b337087a41407030f5525),
    },
    // ServiceCore.EndPointNetwork.RequestSetRankGoalInfoMessage
    ClassRegistration {
        category_id: 397,
        type_id: Uuid::from_u128(0xaf164256d1f0311f9894896845db1f36),
    },
    // ServiceCore.EndPointNetwork.RemoveExpiredCashItemMessage
    ClassRegistration {
        category_id: 398,
        type_id: Uuid::from_u128(0x448113bfd0d83e2fbf0d533103a5e3ac),
    },
    // ServiceCore.EndPointNetwork.RemoveMissionSuccessMessage
    ClassRegistration {
        category_id: 399,
        type_id: Uuid::from_u128(0xb5952ff5361033efb64f7c6d7d575247),
    },
    // ServiceCore.EndPointNetwork.RemoveServerStatusEffectMessage
    ClassRegistration {
        category_id: 400,
        type_id: Uuid::from_u128(0x9360aa9e17b03cb8ad1df1e165393c46),
    },
    // ServiceCore.EndPointNetwork.RequestSortInventoryMessage
    ClassRegistration {
        category_id: 401,
        type_id: Uuid::from_u128(0x984edd04a6ed32428490613f3862d0d7),
    },
    // ServiceCore.EndPointNetwork.ReturnFromQuestMessage
    ClassRegistration {
        category_id: 402,
        type_id: Uuid::from_u128(0x6764d890113e3d4d9d9624c0087489c3),
    },
    // ServiceCore.EndPointNetwork.ApplyMicroPlayEffectsMessage
    ClassRegistration {
        category_id: 403,
        type_id: Uuid::from_u128(0x10c709a928c3328e8dfa6bb6f5ecc0c8),
    },
    // ServiceCore.EndPointNetwork.RewardMissionSuccessMessage
    ClassRegistration {
        category_id: 404,
        type_id: Uuid::from_u128(0xd9550a0691cd35d28dfed10a7b586090),
    },
    // ServiceCore.EndPointNetwork.SecondPasswordMessage
    ClassRegistration {
        category_id: 405,
        type_id: Uuid::from_u128(0x1f3f118d77753b45b608a383ba381ed6),
    },
    // ServiceCore.EndPointNetwork.ClientCmdMessage
    ClassRegistration {
        category_id: 406,
        type_id: Uuid::from_u128(0x0d55ead471fc3e699ce7c7a2addf029f),
    },
    // ServiceCore.EndPointNetwork.DeleteCharacterResultMessage
    ClassRegistration {
        category_id: 407,
        type_id: Uuid::from_u128(0x19af38d6d58a39e5bfc77b5b88c50e4a),
    },
    // ServiceCore.EndPointNetwork.FishingResultMessage
    ClassRegistration {
        category_id: 408,
        type_id: Uuid::from_u128(0x3f44b593eafe3de7a0a805b658e5ee7f),
    },
    // ServiceCore.EndPointNetwork.DyeItemMessage
    ClassRegistration {
        category_id: 409,
        type_id: Uuid::from_u128(0x86468347c23b3ae9be47d9b3f21dd5a0),
    },
    // ServiceCore.EndPointNetwork.DyeItemResultMessage
    ClassRegistration {
        category_id: 410,
        type_id: Uuid::from_u128(0xd653f7a7d409317ea0909f64b1bf200f),
    },
    // ServiceCore.EndPointNetwork.FrameRateMessage
    ClassRegistration {
        category_id: 411,
        type_id: Uuid::from_u128(0x65a8e32c42f830528d877315748cb743),
    },
    // ServiceCore.EndPointNetwork.GoddessProtectionMessage
    ClassRegistration {
        category_id: 412,
        type_id: Uuid::from_u128(0xeb247285b757365387adb407cb02581b),
    },
    // ServiceCore.EndPointNetwork.HasSecondPasswordMessage
    ClassRegistration {
        category_id: 413,
        type_id: Uuid::from_u128(0x95739ac926a7325e9f4bb63c40dc590c),
    },
    // ServiceCore.EndPointNetwork.HideCostumeMessage
    ClassRegistration {
        category_id: 414,
        type_id: Uuid::from_u128(0x80e685d21ace3cf590aa7fda984d169d),
    },
    // ServiceCore.EndPointNetwork.NewRecipesMessage
    ClassRegistration {
        category_id: 415,
        type_id: Uuid::from_u128(0x03a016dd2abd3d98bc2a2a2e490f0632),
    },
    // ServiceCore.EndPointNetwork.NexonSNByNameMessage
    ClassRegistration {
        category_id: 416,
        type_id: Uuid::from_u128(0xcaebf4ec5bca38b8ae2e126cdb5ed34b),
    },
    // ServiceCore.EndPointNetwork.QueryAddBattleInventoryMessage
    ClassRegistration {
        category_id: 417,
        type_id: Uuid::from_u128(0x14c4485ec9173988b207a1e69c8514e0),
    },
    // ServiceCore.EndPointNetwork.QueryCashShopGiftSenderMessage
    ClassRegistration {
        category_id: 418,
        type_id: Uuid::from_u128(0x8904c5be4e743245b0767210083e96e6),
    },
    // ServiceCore.EndPointNetwork.QueryFishingResultMessage
    ClassRegistration {
        category_id: 419,
        type_id: Uuid::from_u128(0x2e8df4019a0330f3853b7cbfabb46ffe),
    },
    // ServiceCore.EndPointNetwork.QueryNewRecipesMessage
    ClassRegistration {
        category_id: 420,
        type_id: Uuid::from_u128(0x30094803b8fa387887fc056e95fc192e),
    },
    // ServiceCore.EndPointNetwork.QueryNexonSNByNameMessage
    ClassRegistration {
        category_id: 421,
        type_id: Uuid::from_u128(0x18f885dbb59a3a49bbe8f468cdfa69a8),
    },
    // ServiceCore.EndPointNetwork.QueryCharacterViewInfoMessage
    ClassRegistration {
        category_id: 422,
        type_id: Uuid::from_u128(0x3c4bbf32f36d31b19ce8613fbd61b313),
    },
    // ServiceCore.EndPointNetwork.RegisterNewRecipesMessage
    ClassRegistration {
        category_id: 423,
        type_id: Uuid::from_u128(0xd57e060241dd3bebab31fec9cf4811ef),
    },
    // ServiceCore.EndPointNetwork.RepairItemMessage
    ClassRegistration {
        category_id: 424,
        type_id: Uuid::from_u128(0xb83c8562c97c343cb421d3cdada26774),
    },
    // ServiceCore.EndPointNetwork.ConnectionRequestMessage
    ClassRegistration {
        category_id: 425,
        type_id: Uuid::from_u128(0x31fc12c3d398355fa15c786e69ccc95f),
    },
    // ServiceCore.EndPointNetwork.GetMailItemCompletedMessage
    ClassRegistration {
        category_id: 426,
        type_id: Uuid::from_u128(0x203f4aafd0733e308f1f4d5458b4e338),
    },
    // ServiceCore.EndPointNetwork.InsertTradeOrderMessage
    ClassRegistration {
        category_id: 427,
        type_id: Uuid::from_u128(0x5dd5d7fb1389353f8bcf7d2fbd9edb82),
    },
    // ServiceCore.EndPointNetwork.InsertTradeOrderResultMessage
    ClassRegistration {
        category_id: 428,
        type_id: Uuid::from_u128(0x7ab66731d8273eeebcec1c50b5a066fc),
    },
    // ServiceCore.EndPointNetwork.InsertTradeTirOrderMessage
    ClassRegistration {
        category_id: 429,
        type_id: Uuid::from_u128(0x97f6218c87333b4bbdd7af26cf089413),
    },
    // ServiceCore.EndPointNetwork.JoinChannelMessage
    ClassRegistration {
        category_id: 430,
        type_id: Uuid::from_u128(0x242dc1c3dab23dcbb4826a85c7c82a39),
    },
    // ServiceCore.EndPointNetwork.LeaveChannelMessage
    ClassRegistration {
        category_id: 431,
        type_id: Uuid::from_u128(0x78edbff4527332f18b377d48d7410321),
    },
    // ServiceCore.EndPointNetwork.PayCoinCompletedMessage
    ClassRegistration {
        category_id: 432,
        type_id: Uuid::from_u128(0x7f105751b6a73c1da7fdbc7a884133d2),
    },
    // ServiceCore.EndPointNetwork.PayCoinMessage
    ClassRegistration {
        category_id: 433,
        type_id: Uuid::from_u128(0xd9bfaf54f8dd35d8afb5ef3348da2c5b),
    },
    // ServiceCore.EndPointNetwork.PvpEndGameMessage
    ClassRegistration {
        category_id: 434,
        type_id: Uuid::from_u128(0x3e774db5ce043fe1ac81fdd35c1ce8eb),
    },
    // ServiceCore.EndPointNetwork.PvpGameHostedMessage
    ClassRegistration {
        category_id: 435,
        type_id: Uuid::from_u128(0xb8b23b051468316e950fb00af304ca2c),
    },
    // ServiceCore.EndPointNetwork.PvpHostInfoMessage
    ClassRegistration {
        category_id: 436,
        type_id: Uuid::from_u128(0x3b54c4d3e9893926b1309312ae448ff2),
    },
    // ServiceCore.EndPointNetwork.PvpMemberInfoMessage
    ClassRegistration {
        category_id: 437,
        type_id: Uuid::from_u128(0x52a61a1645c830e2a8339c2a79e3d2d1),
    },
    // ServiceCore.EndPointNetwork.PvpStartGameMessage
    ClassRegistration {
        category_id: 438,
        type_id: Uuid::from_u128(0x781e16f7a71836be845802bfbec7ddb6),
    },
    // ServiceCore.EndPointNetwork.QueryReservedInfoMessage
    ClassRegistration {
        category_id: 439,
        type_id: Uuid::from_u128(0x0e5fd723eaf13c00aff49360c9367364),
    },
    // ServiceCore.EndPointNetwork.RecommendShipMessage
    ClassRegistration {
        category_id: 440,
        type_id: Uuid::from_u128(0x583a71a9866a308b90fa5e5ca40a825b),
    },
    // ServiceCore.EndPointNetwork.QueryRecommendShipMessage
    ClassRegistration {
        category_id: 441,
        type_id: Uuid::from_u128(0xb5a323d777763d49b0691e93090d766f),
    },
    // ServiceCore.EndPointNetwork.PvpRegisterMessage
    ClassRegistration {
        category_id: 442,
        type_id: Uuid::from_u128(0x352de823b4f4337da212bdfdef5624ce),
    },
    // ServiceCore.EndPointNetwork.ReservedInfoMessage
    ClassRegistration {
        category_id: 443,
        type_id: Uuid::from_u128(0xea3d352343e1370aaad51f0c477c4d5b),
    },
    // ServiceCore.EndPointNetwork.ReturnToShipMessage
    ClassRegistration {
        category_id: 444,
        type_id: Uuid::from_u128(0x6b513c5c0a423a20aff729a95dabed8e),
    },
    // ServiceCore.EndPointNetwork.ReturnToTownMessage
    ClassRegistration {
        category_id: 445,
        type_id: Uuid::from_u128(0x702a1018f1b53c8ead51e46ef0846aed),
    },
    // ServiceCore.EndPointNetwork.CompleteSkillMessage
    ClassRegistration {
        category_id: 446,
        type_id: Uuid::from_u128(0x80e7b3a5a96f3a72825e394db2dbc291),
    },
    // ServiceCore.EndPointNetwork.CostumeUpdateMessage
    ClassRegistration {
        category_id: 447,
        type_id: Uuid::from_u128(0x099b778f141e359db27060c9d2f99bbb),
    },
    // ServiceCore.EndPointNetwork.CreateItemMessage
    ClassRegistration {
        category_id: 448,
        type_id: Uuid::from_u128(0xa9cf236ee5f7308d91c5d4cedd12c11c),
    },
    // ServiceCore.EndPointNetwork.CreateShipMessage
    ClassRegistration {
        category_id: 449,
        type_id: Uuid::from_u128(0x489dfe4e6c3a3cf8b9a07c1686125557),
    },
    // ServiceCore.EndPointNetwork.CustomSectorQuestMessage
    ClassRegistration {
        category_id: 450,
        type_id: Uuid::from_u128(0xf13615f906f23926bfc15ebc2d3fa10d),
    },
    // ServiceCore.EndPointNetwork.DeleteCharacterMessage
    ClassRegistration {
        category_id: 451,
        type_id: Uuid::from_u128(0x65699026e61b38d9a40328178a907f59),
    },
    // ServiceCore.EndPointNetwork.DeleteMailMessage
    ClassRegistration {
        category_id: 452,
        type_id: Uuid::from_u128(0x316f44120b623af69ea6cf98aecce9cb),
    },
    // ServiceCore.EndPointNetwork.DestroyItemMessage
    ClassRegistration {
        category_id: 453,
        type_id: Uuid::from_u128(0x3a0bd92916103d129230c15ff1d6156e),
    },
    // ServiceCore.EndPointNetwork.DestroySlotItemMessage
    ClassRegistration {
        category_id: 454,
        type_id: Uuid::from_u128(0x3a94b5d8d39e3b2f919efc7675997cc0),
    },
    // ServiceCore.EndPointNetwork.DropErgMessage
    ClassRegistration {
        category_id: 455,
        type_id: Uuid::from_u128(0x64b4f44bbe703e40a441a61ceaf133ea),
    },
    // ServiceCore.EndPointNetwork.EquipBundleMessage
    ClassRegistration {
        category_id: 456,
        type_id: Uuid::from_u128(0x6652c42fbf013da09415f390b085e456),
    },
    // ServiceCore.EndPointNetwork.EquipItemMessage
    ClassRegistration {
        category_id: 457,
        type_id: Uuid::from_u128(0xa65518163ca138a28ec07ecba2ac7f36),
    },
    // ServiceCore.EndPointNetwork.EquipmentInfoMessage
    ClassRegistration {
        category_id: 458,
        type_id: Uuid::from_u128(0x9df997b4ce70355886a17ee1ea91325c),
    },
    // ServiceCore.EndPointNetwork.ExecuteNpcServerCommandMessage
    ClassRegistration {
        category_id: 459,
        type_id: Uuid::from_u128(0x048fdbb2e4523c5fb0b2cfc80994e365),
    },
    // ServiceCore.EndPointNetwork.FinishGameMessage
    ClassRegistration {
        category_id: 460,
        type_id: Uuid::from_u128(0x4bc1c4f007af34499246f7a5e68f76a2),
    },
    // ServiceCore.EndPointNetwork.GameStartedMessage
    ClassRegistration {
        category_id: 461,
        type_id: Uuid::from_u128(0xad7c56b69ef639a7879aec1f9ef3e31f),
    },
    // ServiceCore.EndPointNetwork.GameStartGrantedMessage
    ClassRegistration {
        category_id: 462,
        type_id: Uuid::from_u128(0xdc3acd0b69463b6cbcaf29057e0dfc59),
    },
    // ServiceCore.EndPointNetwork.GetItemMessage
    ClassRegistration {
        category_id: 463,
        type_id: Uuid::from_u128(0xcd70d18ddc3b3053b9fe6efbc76d28bb),
    },
    // ServiceCore.EndPointNetwork.GetMailItemMessage
    ClassRegistration {
        category_id: 464,
        type_id: Uuid::from_u128(0x5c2de52aae2c3c6e906014ca27b9eef3),
    },
    // ServiceCore.EndPointNetwork.HackShieldRequestMessage
    ClassRegistration {
        category_id: 465,
        type_id: Uuid::from_u128(0xd7a4105eab063da39c1c1b0846caef3c),
    },
    // ServiceCore.EndPointNetwork.HackShieldRespondMessage
    ClassRegistration {
        category_id: 466,
        type_id: Uuid::from_u128(0xba58f43282c030c2921f3dc3573a9248),
    },
    // ServiceCore.EndPointNetwork.HostInfoMessage
    ClassRegistration {
        category_id: 467,
        type_id: Uuid::from_u128(0x5d2864e11acf3ce58a945df5b30936a3),
    },
    // ServiceCore.EndPointNetwork.InventoryInfoMessage
    ClassRegistration {
        category_id: 468,
        type_id: Uuid::from_u128(0xbdc965b5aca13b62b222279e96683d93),
    },
    // ServiceCore.EndPointNetwork.JoinShipMessage
    ClassRegistration {
        category_id: 469,
        type_id: Uuid::from_u128(0x47c79785d68934ff8bef15d468628eb7),
    },
    // ServiceCore.EndPointNetwork.KickMessage
    ClassRegistration {
        category_id: 470,
        type_id: Uuid::from_u128(0x99ce5fa11d973ae89814bc17ecbda388),
    },
    // ServiceCore.EndPointNetwork.KickedMessage
    ClassRegistration {
        category_id: 471,
        type_id: Uuid::from_u128(0x4df7486768cb34318177ce7549890ddd),
    },
    // ServiceCore.EndPointNetwork.KilledMessage
    ClassRegistration {
        category_id: 472,
        type_id: Uuid::from_u128(0x98f39b7818843a0bb536c37c93c80d08),
    },
    // ServiceCore.EndPointNetwork.LaunchShipGrantedMessage
    ClassRegistration {
        category_id: 473,
        type_id: Uuid::from_u128(0x78df701a83a333f5be948d27ecb6e23b),
    },
    // ServiceCore.EndPointNetwork.LearningSkillChangedMessage
    ClassRegistration {
        category_id: 474,
        type_id: Uuid::from_u128(0xc85ffbe14cb1363096b845952fab8084),
    },
    // ServiceCore.EndPointNetwork.LearnSkillMessage
    ClassRegistration {
        category_id: 475,
        type_id: Uuid::from_u128(0x3ec0056495e33507b9dca5a185215b19),
    },
    // ServiceCore.EndPointNetwork.LevelUpMessage
    ClassRegistration {
        category_id: 476,
        type_id: Uuid::from_u128(0x23314733956536b4a3828042ea30debe),
    },
    // ServiceCore.EndPointNetwork.LoggedOutMessage
    ClassRegistration {
        category_id: 477,
        type_id: Uuid::from_u128(0x1a508d7faeb73e2bbb082aa14a2d6d90),
    },
    // ServiceCore.EndPointNetwork.UserLoginMessage
    ClassRegistration {
        category_id: 478,
        type_id: Uuid::from_u128(0x47f6f00c0bd33059afad84a872ed7399),
    },
    // ServiceCore.EndPointNetwork.CharacterListMessage
    ClassRegistration {
        category_id: 479,
        type_id: Uuid::from_u128(0xf2750037d9ab3b349697e1626e224040),
    },
    // ServiceCore.EndPointNetwork.SingleCharacterMessage
    ClassRegistration {
        category_id: 480,
        type_id: Uuid::from_u128(0x6c9dd9f6232f3deda19ed4d5fdf32b84),
    },
    // ServiceCore.EndPointNetwork.SelectCharacterMessage
    ClassRegistration {
        category_id: 481,
        type_id: Uuid::from_u128(0x1f34c6725581353f9944d5ab72e808a5),
    },
    // ServiceCore.EndPointNetwork.CreateCharacterMessage
    ClassRegistration {
        category_id: 482,
        type_id: Uuid::from_u128(0x677216687ebf37919ccd53bb01933e99),
    },
    // ServiceCore.EndPointNetwork.LoginOkMessage
    ClassRegistration {
        category_id: 483,
        type_id: Uuid::from_u128(0xe301ee1410c238868533fbba9ef1cbe1),
    },
    // ServiceCore.EndPointNetwork.LoginFailMessage
    ClassRegistration {
        category_id: 484,
        type_id: Uuid::from_u128(0x5b8a16caaaef325fa041f337dcf14f31),
    },
    // ServiceCore.EndPointNetwork.CreateCharacterFailMessage
    ClassRegistration {
        category_id: 485,
        type_id: Uuid::from_u128(0x8676de0575e83b6ab6a481065f352b9b),
    },
    // ServiceCore.EndPointNetwork.PurchaseCharacterSlotMessage
    ClassRegistration {
        category_id: 486,
        type_id: Uuid::from_u128(0xede9320c872a3add91f12270149435e8),
    },
    // ServiceCore.EndPointNetwork.QueryCharacterNameChangeMessage
    ClassRegistration {
        category_id: 487,
        type_id: Uuid::from_u128(0xfee2e12b77d435bbab7a2f0f218ea8af),
    },
    // ServiceCore.EndPointNetwork.CharacterNameChangeMessage
    ClassRegistration {
        category_id: 488,
        type_id: Uuid::from_u128(0x42b1e644ce3539ebae228cb251f13379),
    },
    // ServiceCore.EndPointNetwork.SelectOrdersChangeMessage
    ClassRegistration {
        category_id: 489,
        type_id: Uuid::from_u128(0xd7d63e4997423287a8157a0483760e81),
    },
    // ServiceCore.EndPointNetwork.TransferValuesListMessage
    ClassRegistration {
        category_id: 490,
        type_id: Uuid::from_u128(0x93a960de30e23286a30b4c93e596c2cd),
    },
    // ServiceCore.EndPointNetwork.TransferValuesSetMessage
    ClassRegistration {
        category_id: 491,
        type_id: Uuid::from_u128(0x999f453ebe633b83acc265ba8262df3e),
    },
    // ServiceCore.EndPointNetwork.TransferValuesResultMessage
    ClassRegistration {
        category_id: 492,
        type_id: Uuid::from_u128(0xa2d2ec3830fd3483bd9240a3965b38e6),
    },
    // ServiceCore.EndPointNetwork.LogOutMessage
    ClassRegistration {
        category_id: 493,
        type_id: Uuid::from_u128(0x312f4152ab0d3ac299fafffa1e3f7e5e),
    },
    // ServiceCore.EndPointNetwork.MailInfoMessage
    ClassRegistration {
        category_id: 494,
        type_id: Uuid::from_u128(0x720d93db8a10349d9f3181ae382857a6),
    },
    // ServiceCore.EndPointNetwork.MailListMessage
    ClassRegistration {
        category_id: 495,
        type_id: Uuid::from_u128(0x743934678b2a31009eb1b5a34ec3310b),
    },
    // ServiceCore.EndPointNetwork.MailReceivedMessage
    ClassRegistration {
        category_id: 496,
        type_id: Uuid::from_u128(0x67e4050be4023a58be39fce090eb16dd),
    },
    // ServiceCore.EndPointNetwork.MailSentMessage
    ClassRegistration {
        category_id: 497,
        type_id: Uuid::from_u128(0x06e86b5bc00a335ea28f92f6a517b156),
    },
    // ServiceCore.EndPointNetwork.MemberInfoMessage
    ClassRegistration {
        category_id: 498,
        type_id: Uuid::from_u128(0xb89c49cb0aa1308da0c59fe9bb1f58ed),
    },
    // ServiceCore.EndPointNetwork.ModifyStoryStatusMessage
    ClassRegistration {
        category_id: 499,
        type_id: Uuid::from_u128(0xfb45b55bc26f3b2c8c8db28be6e50e36),
    },
    // ServiceCore.EndPointNetwork.ModifyStoryVariableMessage
    ClassRegistration {
        category_id: 500,
        type_id: Uuid::from_u128(0xdea5a4dc55e2348289cb8190a091af8d),
    },
    // ServiceCore.EndPointNetwork.MonsterKilledMessage
    ClassRegistration {
        category_id: 501,
        type_id: Uuid::from_u128(0x8cfddc5b72e03490bb441ecf1a813468),
    },
    // ServiceCore.EndPointNetwork.MoveInventoryItemMessage
    ClassRegistration {
        category_id: 502,
        type_id: Uuid::from_u128(0x7c957f24e8d7338fa93c97bc6903f320),
    },
    // ServiceCore.EndPointNetwork.NextSectorMessage
    ClassRegistration {
        category_id: 503,
        type_id: Uuid::from_u128(0xab1d44c6b34b348ba157d63038d910e7),
    },
    // ServiceCore.EndPointNetwork.NoticeMessage
    ClassRegistration {
        category_id: 504,
        type_id: Uuid::from_u128(0x1fe4016f8dea3d66ab9c7193862b8aee),
    },
    // ServiceCore.EndPointNetwork.NoticeShutDownMessage
    ClassRegistration {
        category_id: 505,
        type_id: Uuid::from_u128(0x88cb090bcdc1367c9397698aabbb3e8a),
    },
    // ServiceCore.EndPointNetwork.NotifyForceStartMessage
    ClassRegistration {
        category_id: 506,
        type_id: Uuid::from_u128(0x426ccd05ac013b00b5f6f43ca6c608e8),
    },
    // ServiceCore.EndPointNetwork.NpcListMessage
    ClassRegistration {
        category_id: 507,
        type_id: Uuid::from_u128(0x35be098a89fe3185a8f31cf91f5c07c1),
    },
    // ServiceCore.EndPointNetwork.NpcTalkMessage
    ClassRegistration {
        category_id: 508,
        type_id: Uuid::from_u128(0x859ce7b4cfd136959a73b5c094d7085e),
    },
    // ServiceCore.EndPointNetwork.PartyChatMessage
    ClassRegistration {
        category_id: 509,
        type_id: Uuid::from_u128(0x2fa48705fd8631fbb14f220b2e2473c3),
    },
    // ServiceCore.EndPointNetwork.PartyChatSendMessage
    ClassRegistration {
        category_id: 510,
        type_id: Uuid::from_u128(0xbfe08582060134f4b14940c77976298c),
    },
    // ServiceCore.EndPointNetwork.PartyInfoMessage
    ClassRegistration {
        category_id: 511,
        type_id: Uuid::from_u128(0x78d2aa7713f9302fb9fe67ca096b92f3),
    },
    // ServiceCore.EndPointNetwork.PartyInvitationAcceptFailedMessage
    ClassRegistration {
        category_id: 512,
        type_id: Uuid::from_u128(0x5f443853582d3ff99f7a2320ce2a3d21),
    },
    // ServiceCore.EndPointNetwork.PartyInvitationAcceptMessage
    ClassRegistration {
        category_id: 513,
        type_id: Uuid::from_u128(0xc0c444a8a15d338799760d92317a3016),
    },
    // ServiceCore.EndPointNetwork.PartyInvitationRejectedMessage
    ClassRegistration {
        category_id: 514,
        type_id: Uuid::from_u128(0xf14e58a48408325e98958e4ee76b1bc2),
    },
    // ServiceCore.EndPointNetwork.PartyInvitationRejectMessage
    ClassRegistration {
        category_id: 515,
        type_id: Uuid::from_u128(0x0d393c289f51354a96725b0c8c8c2c92),
    },
    // ServiceCore.EndPointNetwork.PartyInvitedMessage
    ClassRegistration {
        category_id: 516,
        type_id: Uuid::from_u128(0x01280cf140493e1ab428a73e3de11091),
    },
    // ServiceCore.EndPointNetwork.PartyInviteMessage
    ClassRegistration {
        category_id: 517,
        type_id: Uuid::from_u128(0xf7d434f2e9ef37e79d744d005127df1a),
    },
    // ServiceCore.EndPointNetwork.LeavePartyMessage
    ClassRegistration {
        category_id: 518,
        type_id: Uuid::from_u128(0x4aad8f98e2653b91b7fcf25caed054b9),
    },
    // ServiceCore.EndPointNetwork.PartyOptionMessage
    ClassRegistration {
        category_id: 519,
        type_id: Uuid::from_u128(0x81a2612101083840b2a5d1c2ff1dee2c),
    },
    // ServiceCore.EndPointNetwork.PickErgMessage
    ClassRegistration {
        category_id: 520,
        type_id: Uuid::from_u128(0x2e9ee02b643930799834932286dfbb4b),
    },
    // ServiceCore.EndPointNetwork.PingMessage
    ClassRegistration {
        category_id: 521,
        type_id: Uuid::from_u128(0xf4889dff7be13380a75d16931cf0367f),
    },
    // ServiceCore.EndPointNetwork.PlayerKilledMessage
    ClassRegistration {
        category_id: 522,
        type_id: Uuid::from_u128(0x71d3965f66a231f1aa1590d54c7c7882),
    },
    // ServiceCore.EndPointNetwork.PongMessage
    ClassRegistration {
        category_id: 523,
        type_id: Uuid::from_u128(0xe7c289c5c6c3356085222b09a473e7d9),
    },
    // ServiceCore.EndPointNetwork.PropBrokenMessage
    ClassRegistration {
        category_id: 524,
        type_id: Uuid::from_u128(0x15af6960d57a3be7820cd8825a163a0b),
    },
    // ServiceCore.EndPointNetwork.PutErgMessage
    ClassRegistration {
        category_id: 525,
        type_id: Uuid::from_u128(0xd7f2e4f911e7369ca4dd70d46756e0ad),
    },
    // ServiceCore.EndPointNetwork.QueryAPMessage
    ClassRegistration {
        category_id: 526,
        type_id: Uuid::from_u128(0xf061ae98f5c8333d82ca3b217e268120),
    },
    // ServiceCore.EndPointNetwork.QueryCharacterCommonInfoMessage
    ClassRegistration {
        category_id: 527,
        type_id: Uuid::from_u128(0xc9cda3f68b673c69ba31ba2fe827fd7b),
    },
    // ServiceCore.EndPointNetwork.QueryEquipmentMessage
    ClassRegistration {
        category_id: 528,
        type_id: Uuid::from_u128(0x03e821c68e6b3cd1acc7605b0686b112),
    },
    // ServiceCore.EndPointNetwork.QueryInventoryMessage
    ClassRegistration {
        category_id: 529,
        type_id: Uuid::from_u128(0xefe95078ef7e34eb82e12daf3280a576),
    },
    // ServiceCore.EndPointNetwork.QueryMailListMessage
    ClassRegistration {
        category_id: 530,
        type_id: Uuid::from_u128(0x6f7864f65e4933a3a8996a6801411695),
    },
    // ServiceCore.EndPointNetwork.QueryMailInfoMessage
    ClassRegistration {
        category_id: 531,
        type_id: Uuid::from_u128(0x55a3b131002a3a3a9e6514132b97951d),
    },
    // ServiceCore.EndPointNetwork.MoveToNextSectorMessage
    ClassRegistration {
        category_id: 532,
        type_id: Uuid::from_u128(0x67aefac8659d385aa39a03e3d1dce102),
    },
    // ServiceCore.EndPointNetwork.QueryNpcListMessage
    ClassRegistration {
        category_id: 533,
        type_id: Uuid::from_u128(0x6d029dfe89e532d0aa7927d298e3b87c),
    },
    // ServiceCore.EndPointNetwork.QueryNpcTalkMessage
    ClassRegistration {
        category_id: 534,
        type_id: Uuid::from_u128(0x986c6c40c9b63aed93030b5679aa8f53),
    },
    // ServiceCore.EndPointNetwork.QueryQuestProgressMessage
    ClassRegistration {
        category_id: 535,
        type_id: Uuid::from_u128(0x5b634b00c047339ea4feb0ef8e3b9a2e),
    },
    // ServiceCore.EndPointNetwork.QueryQuestsMessage
    ClassRegistration {
        category_id: 536,
        type_id: Uuid::from_u128(0xdfd777d8f7f63310a5af49f067e078ec),
    },
    // ServiceCore.EndPointNetwork.QuerySectorEntitiesMessage
    ClassRegistration {
        category_id: 537,
        type_id: Uuid::from_u128(0xbebc6f30261c3bb89ac3e2b6ee8a0b51),
    },
    // ServiceCore.EndPointNetwork.QueryShipInfoMessage
    ClassRegistration {
        category_id: 538,
        type_id: Uuid::from_u128(0x6800ce3b7560306295524c0472cf2e69),
    },
    // ServiceCore.EndPointNetwork.QueryShipListMessage
    ClassRegistration {
        category_id: 539,
        type_id: Uuid::from_u128(0x0cc952a0f2c73b80af70ab66b55a7f6c),
    },
    // ServiceCore.EndPointNetwork.QuerySkillListMessage
    ClassRegistration {
        category_id: 540,
        type_id: Uuid::from_u128(0x860e6484d1b534a9941270807bbbf79c),
    },
    // ServiceCore.EndPointNetwork.QueryStatMessage
    ClassRegistration {
        category_id: 541,
        type_id: Uuid::from_u128(0xe5d87b1642c03c9bbff450d7bcbd94d0),
    },
    // ServiceCore.EndPointNetwork.QueryStoryGuideMessage
    ClassRegistration {
        category_id: 542,
        type_id: Uuid::from_u128(0x2bc3c01405f83e6d8c08aeca1928b979),
    },
    // ServiceCore.EndPointNetwork.QueryStoryLinesMessage
    ClassRegistration {
        category_id: 543,
        type_id: Uuid::from_u128(0x7aecc88f91e7328fb8ab47f71ef16e29),
    },
    // ServiceCore.EndPointNetwork.QueryStoryVariablesMessage
    ClassRegistration {
        category_id: 544,
        type_id: Uuid::from_u128(0x4e8b97d8b66b300c8d6cd0a67c37946a),
    },
    // ServiceCore.EndPointNetwork.QuestListMessage
    ClassRegistration {
        category_id: 545,
        type_id: Uuid::from_u128(0xd9e1dd57a57a35d68c8b26bc6edc2059),
    },
    // ServiceCore.EndPointNetwork.QuestProgressMessage
    ClassRegistration {
        category_id: 546,
        type_id: Uuid::from_u128(0xc229a0f54eda3bbca4be61f3aad3f82b),
    },
    // ServiceCore.EndPointNetwork.QuestReinforceMessage
    ClassRegistration {
        category_id: 547,
        type_id: Uuid::from_u128(0xf0e1c95e3e7a3969b348bea1acc7c9b5),
    },
    // ServiceCore.EndPointNetwork.AddReinforcementMessage
    ClassRegistration {
        category_id: 548,
        type_id: Uuid::from_u128(0xf68c95c7d7753e8bbc42bd4a91326cec),
    },
    // ServiceCore.EndPointNetwork.QuestReinforceGrantedMessage
    ClassRegistration {
        category_id: 549,
        type_id: Uuid::from_u128(0x645bed9e93203c4a8d303b331396f2fc),
    },
    // ServiceCore.EndPointNetwork.QuestSuccessSceneStartMessage
    ClassRegistration {
        category_id: 550,
        type_id: Uuid::from_u128(0x62db5582deb334e798e23fd6692deca6),
    },
    // ServiceCore.EndPointNetwork.QuickSlotInfoMessage
    ClassRegistration {
        category_id: 551,
        type_id: Uuid::from_u128(0xedc4290e196c3f17903fd0f3317e47d9),
    },
    // ServiceCore.EndPointNetwork.RagdollKickedMessage
    ClassRegistration {
        category_id: 552,
        type_id: Uuid::from_u128(0xe7ae9d95805f3478b19eb03c14234ee7),
    },
    // ServiceCore.EndPointNetwork.ReadyMessage
    ClassRegistration {
        category_id: 553,
        type_id: Uuid::from_u128(0xca4b672b2bbd315382adf8fa0a917dfb),
    },
    // ServiceCore.EndPointNetwork.RegisterServerMessage
    ClassRegistration {
        category_id: 554,
        type_id: Uuid::from_u128(0xb8a9e8c0377b35adb3bebb911152a565),
    },
    // ServiceCore.EndPointNetwork.PlayerRevivedMessage
    ClassRegistration {
        category_id: 555,
        type_id: Uuid::from_u128(0x0441624d0d613f4d8194090df4655d18),
    },
    // ServiceCore.EndPointNetwork.ReleaseLearningSkillMessage
    ClassRegistration {
        category_id: 556,
        type_id: Uuid::from_u128(0x8e16be04062a34e98c5f298491ea5312),
    },
    // ServiceCore.EndPointNetwork.ReloadNpcScriptMessage
    ClassRegistration {
        category_id: 557,
        type_id: Uuid::from_u128(0x292d89c6610631cb802ebd8f8e8a89ae),
    },
    // ServiceCore.EndPointNetwork.RequestCraftMessage
    ClassRegistration {
        category_id: 558,
        type_id: Uuid::from_u128(0x7ba36e6f593530d7b5a67bbcda0acce7),
    },
    // ServiceCore.EndPointNetwork.RequestShutDownMessage
    ClassRegistration {
        category_id: 559,
        type_id: Uuid::from_u128(0x214f668d30323ab180596dc61caaed61),
    },
    // ServiceCore.EndPointNetwork.RequestStoryStatusMessage
    ClassRegistration {
        category_id: 560,
        type_id: Uuid::from_u128(0xb1252ee6965937bcb3925d4d03138f95),
    },
    // ServiceCore.EndPointNetwork.ReturnMailMessage
    ClassRegistration {
        category_id: 561,
        type_id: Uuid::from_u128(0x6eb38330040937d39c8b8062e9f45d48),
    },
    // ServiceCore.EndPointNetwork.SecondPasswordResultMessage
    ClassRegistration {
        category_id: 562,
        type_id: Uuid::from_u128(0xb2b1f8d64c60325e854f2f73550d8e9b),
    },
    // ServiceCore.EndPointNetwork.SectorPropListMessage
    ClassRegistration {
        category_id: 563,
        type_id: Uuid::from_u128(0x64e27db448c93507a3669bbb6b1b6a79),
    },
    // ServiceCore.EndPointNetwork.SelectButtonMessage
    ClassRegistration {
        category_id: 564,
        type_id: Uuid::from_u128(0xd5dda62d44c13c6a890672164e564921),
    },
    // ServiceCore.EndPointNetwork.SelectTitleMessage
    ClassRegistration {
        category_id: 565,
        type_id: Uuid::from_u128(0xa5b51059e93139efa55eb5e52b2b830a),
    },
    // ServiceCore.EndPointNetwork.SellItemMessage
    ClassRegistration {
        category_id: 566,
        type_id: Uuid::from_u128(0x6266904285a8354c9cabccae3c2becae),
    },
    // ServiceCore.EndPointNetwork.SendMailMessage
    ClassRegistration {
        category_id: 567,
        type_id: Uuid::from_u128(0xfc081bf4efe6312b9de9b6c07f604817),
    },
    // ServiceCore.EndPointNetwork.ServerCmdMessage
    ClassRegistration {
        category_id: 568,
        type_id: Uuid::from_u128(0xe75e7fc4db1437818a5fe403703a45b0),
    },
    // ServiceCore.EndPointNetwork.SetLearningSkillMessage
    ClassRegistration {
        category_id: 569,
        type_id: Uuid::from_u128(0xee8c63c010ff329ca90c46cdd05af8c2),
    },
    // ServiceCore.EndPointNetwork.SetQuoteMessage
    ClassRegistration {
        category_id: 570,
        type_id: Uuid::from_u128(0x0800e57676d7354789724ecd62275bcd),
    },
    // ServiceCore.EndPointNetwork.SetRaidPartyMessage
    ClassRegistration {
        category_id: 571,
        type_id: Uuid::from_u128(0xc0307f1108dd3708b295f277ecb26c6f),
    },
    // ServiceCore.EndPointNetwork.SetSecondPasswordMessage
    ClassRegistration {
        category_id: 572,
        type_id: Uuid::from_u128(0x0c9611cc7fe03aea8e2b35b3d8904786),
    },
    // ServiceCore.EndPointNetwork.SetSpSkillMessage
    ClassRegistration {
        category_id: 573,
        type_id: Uuid::from_u128(0x679e2982a00538378ba297c6acbea6f4),
    },
    // ServiceCore.EndPointNetwork.SetStoryLineStatusMessage
    ClassRegistration {
        category_id: 574,
        type_id: Uuid::from_u128(0x92e441faadce304eaeb2a77fb76d2900),
    },
    // ServiceCore.EndPointNetwork.SharingStartMessage
    ClassRegistration {
        category_id: 575,
        type_id: Uuid::from_u128(0x448c697aea333fbcbc4fc0232b7ff124),
    },
    // ServiceCore.EndPointNetwork.SharingCheckMessage
    ClassRegistration {
        category_id: 576,
        type_id: Uuid::from_u128(0xb54c44e67f03356c98430ca9febce0c8),
    },
    // ServiceCore.EndPointNetwork.SharingResponseMessage
    ClassRegistration {
        category_id: 577,
        type_id: Uuid::from_u128(0x23544d4f028a3ec18968dcc832b384b2),
    },
    // ServiceCore.EndPointNetwork.ShipInfoMessage
    ClassRegistration {
        category_id: 578,
        type_id: Uuid::from_u128(0x0880a348d0f935b7a58d388a25323ccc),
    },
    // ServiceCore.EndPointNetwork.ShipListMessage
    ClassRegistration {
        category_id: 579,
        type_id: Uuid::from_u128(0x15bc8a0f3d0a322fb79fd9162d170371),
    },
    // ServiceCore.EndPointNetwork.ShipNotLaunchedMessage
    ClassRegistration {
        category_id: 580,
        type_id: Uuid::from_u128(0xc45b86f05dea3f46b33fbe3e461cb061),
    },
    // ServiceCore.EndPointNetwork.ShipOptionMessage
    ClassRegistration {
        category_id: 581,
        type_id: Uuid::from_u128(0xddd850bb54143465818519ab89dcd16e),
    },
    // ServiceCore.EndPointNetwork.SkillCompletionMessage
    ClassRegistration {
        category_id: 582,
        type_id: Uuid::from_u128(0x2b5614fc88303aeca03fc33c270de9f1),
    },
    // ServiceCore.EndPointNetwork.SkillListMessage
    ClassRegistration {
        category_id: 583,
        type_id: Uuid::from_u128(0xdd3b94a964da381ab2e70fad58835746),
    },
    // ServiceCore.EndPointNetwork.SpSkillMessage
    ClassRegistration {
        category_id: 584,
        type_id: Uuid::from_u128(0x3c9ded873ca13dc886c18b5dcf6a7b27),
    },
    // ServiceCore.EndPointNetwork.StartAutoFishingMessage
    ClassRegistration {
        category_id: 585,
        type_id: Uuid::from_u128(0xc044ddc4526f3a19a6415759b0eea46e),
    },
    // ServiceCore.EndPointNetwork.AutoFishingStartedMessage
    ClassRegistration {
        category_id: 586,
        type_id: Uuid::from_u128(0x8107c4def37834089678e70b52842b18),
    },
    // ServiceCore.EndPointNetwork.CatchAutoFishMessage
    ClassRegistration {
        category_id: 587,
        type_id: Uuid::from_u128(0x190b1699aaa03afc9336f2866873818c),
    },
    // ServiceCore.EndPointNetwork.LostAutoFishMessage
    ClassRegistration {
        category_id: 588,
        type_id: Uuid::from_u128(0xedd42fa5e1c130788d1a93acddd46070),
    },
    // ServiceCore.EndPointNetwork.AutoFishingDeniedMessage
    ClassRegistration {
        category_id: 589,
        type_id: Uuid::from_u128(0xf7821b6278e23c96a357700c416bdc78),
    },
    // ServiceCore.EndPointNetwork.CancelAutoFishingMessage
    ClassRegistration {
        category_id: 590,
        type_id: Uuid::from_u128(0x52832d89f98936ae94455733ebfe3c82),
    },
    // ServiceCore.EndPointNetwork.StartGameMessage
    ClassRegistration {
        category_id: 591,
        type_id: Uuid::from_u128(0x7c04c215b83c3053b80c966064258660),
    },
    // ServiceCore.EndPointNetwork.StartTradeSessionMessage
    ClassRegistration {
        category_id: 592,
        type_id: Uuid::from_u128(0x99350e052420364aa3bc4be82c6f1cf7),
    },
    // ServiceCore.EndPointNetwork.StartTradeSessionResultMessage
    ClassRegistration {
        category_id: 593,
        type_id: Uuid::from_u128(0xf8356e37a4753c43b6c6af65d1a2eb74),
    },
    // ServiceCore.EndPointNetwork.EndTradeSessionMessage
    ClassRegistration {
        category_id: 594,
        type_id: Uuid::from_u128(0x84675e2c42fe38959f1fb913dd2d1918),
    },
    // ServiceCore.EndPointNetwork.CharacterViewInfoMessage
    ClassRegistration {
        category_id: 595,
        type_id: Uuid::from_u128(0x9f614236e50d35ca88ec3d4151a51a31),
    },
    // ServiceCore.EndPointNetwork.StatusEffectUpdated
    ClassRegistration {
        category_id: 596,
        type_id: Uuid::from_u128(0x66f872d1ed92357d862ed6bbc3022c7a),
    },
    // ServiceCore.EndPointNetwork.MicroStatusEffectUpdated
    ClassRegistration {
        category_id: 597,
        type_id: Uuid::from_u128(0xcdf0ee5c80fb322c98947f3525de2642),
    },
    // ServiceCore.EndPointNetwork.StoryLinesMessage
    ClassRegistration {
        category_id: 598,
        type_id: Uuid::from_u128(0x5498b052d11930e282241e2b990d4ef8),
    },
    // ServiceCore.EndPointNetwork.StoryVariablesMessage
    ClassRegistration {
        category_id: 599,
        type_id: Uuid::from_u128(0xde76b127f09d369a800826af3e5c8383),
    },
    // ServiceCore.EndPointNetwork.SwitchChannelMessage
    ClassRegistration {
        category_id: 600,
        type_id: Uuid::from_u128(0xb612281d301b3d31bbb03840eca54f75),
    },
    // ServiceCore.EndPointNetwork.SynthesisItemMessage
    ClassRegistration {
        category_id: 601,
        type_id: Uuid::from_u128(0x74b91fb78fc3332a9428f89789bdaada),
    },
    // ServiceCore.EndPointNetwork.SystemMessage
    ClassRegistration {
        category_id: 602,
        type_id: Uuid::from_u128(0xe3fb23ddbce03c81b40416e93f7bea38),
    },
    // ServiceCore.EndPointNetwork.TeacherAssistJoin
    ClassRegistration {
        category_id: 603,
        type_id: Uuid::from_u128(0x663feb70722339e6914d221866621bda),
    },
    // ServiceCore.EndPointNetwork.TeacherRequest
    ClassRegistration {
        category_id: 604,
        type_id: Uuid::from_u128(0xddd0958d298f3590b69d252fee9f4eab),
    },
    // ServiceCore.EndPointNetwork.TeacherRequestNotice
    ClassRegistration {
        category_id: 605,
        type_id: Uuid::from_u128(0xcf6a33d3558834beadc908e46ac353e7),
    },
    // ServiceCore.EndPointNetwork.TeacherRequestRespond
    ClassRegistration {
        category_id: 606,
        type_id: Uuid::from_u128(0x25b20b0f75a13c7084c339c377aedbe6),
    },
    // ServiceCore.EndPointNetwork.TeacherRequestResult
    ClassRegistration {
        category_id: 607,
        type_id: Uuid::from_u128(0xd190eca229693f8a806a4493dc1037a6),
    },
    // ServiceCore.EndPointNetwork.TestMessage
    ClassRegistration {
        category_id: 608,
        type_id: Uuid::from_u128(0x779d534e3f453cd5827a21f8bb581e39),
    },
    // ServiceCore.EndPointNetwork.CharacterCommonInfoMessage
    ClassRegistration {
        category_id: 609,
        type_id: Uuid::from_u128(0x9756914f67533228b0b838b30dc1a612),
    },
    // ServiceCore.EndPointNetwork.TitleListMessage
    ClassRegistration {
        category_id: 610,
        type_id: Uuid::from_u128(0xe537b12bc3c83ae2816edf0465b4883b),
    },
    // ServiceCore.EndPointNetwork.PvpUnregisterMessage
    ClassRegistration {
        category_id: 611,
        type_id: Uuid::from_u128(0x28670757a5f23ad19bde5031f6606d6c),
    },
    // ServiceCore.EndPointNetwork.TownCampfireEffectCSMessage
    ClassRegistration {
        category_id: 612,
        type_id: Uuid::from_u128(0x3ccf332f882c337c8ef4aabb99c390be),
    },
    // ServiceCore.EndPointNetwork.TriggerEventMessage
    ClassRegistration {
        category_id: 613,
        type_id: Uuid::from_u128(0xe12510d6caa63a01bcb510a40a6b6991),
    },
    // ServiceCore.EndPointNetwork.UnequippablePartsInfoMessage
    ClassRegistration {
        category_id: 614,
        type_id: Uuid::from_u128(0xd0ac3a7385f738bbba0768bec5d8ae21),
    },
    // ServiceCore.EndPointNetwork.UnregisterNewRecipesMessage
    ClassRegistration {
        category_id: 615,
        type_id: Uuid::from_u128(0xc820540df3ee342abb7c03ad958dd7fb),
    },
    // ServiceCore.EndPointNetwork.UpdateBattleInventoryMessage
    ClassRegistration {
        category_id: 616,
        type_id: Uuid::from_u128(0x64c28555d00d38a1b831489f9a967059),
    },
    // ServiceCore.EndPointNetwork.UpdateInventoryInfoMessage
    ClassRegistration {
        category_id: 617,
        type_id: Uuid::from_u128(0xabacd3bc1a843570a62cf1b0007675fb),
    },
    // ServiceCore.EndPointNetwork.UpdateShipMessage
    ClassRegistration {
        category_id: 618,
        type_id: Uuid::from_u128(0xc87e7f92c8c236b183e8bc9fbd04e5d4),
    },
    // ServiceCore.EndPointNetwork.UpdateStatMessage
    ClassRegistration {
        category_id: 619,
        type_id: Uuid::from_u128(0xa1407a79535334559a2c859f8825c200),
    },
    // ServiceCore.EndPointNetwork.UpdateStorageInfoMessage
    ClassRegistration {
        category_id: 620,
        type_id: Uuid::from_u128(0x8eeec06748d438529e5e6a463040bf0c),
    },
    // ServiceCore.EndPointNetwork.UpdateStorageStatusMessage
    ClassRegistration {
        category_id: 621,
        type_id: Uuid::from_u128(0xf9ec037c645a373e87f54d44be0598fb),
    },
    // ServiceCore.EndPointNetwork.UpdateStoryGuideMessage
    ClassRegistration {
        category_id: 622,
        type_id: Uuid::from_u128(0x4f66123411613db98127877ed7807435),
    },
    // ServiceCore.EndPointNetwork.UpdateStoryStatusMessage
    ClassRegistration {
        category_id: 623,
        type_id: Uuid::from_u128(0x66fe3283a8493df0a4bba876921062d1),
    },
    // ServiceCore.EndPointNetwork.UseConsumableMessage
    ClassRegistration {
        category_id: 624,
        type_id: Uuid::from_u128(0x8cf9f82f043331a7b88dd61b71ee61ee),
    },
    // ServiceCore.EndPointNetwork.UseCrateItemResultMessage
    ClassRegistration {
        category_id: 625,
        type_id: Uuid::from_u128(0xe02bc0e98ee73d22a2214f0292db04cb),
    },
    // ServiceCore.EndPointNetwork.UseExtraStorageMessage
    ClassRegistration {
        category_id: 626,
        type_id: Uuid::from_u128(0xe9cd479d3b6b3d9e850ec228d89b7aa7),
    },
    // ServiceCore.EndPointNetwork.UseInventoryItemOkMessage
    ClassRegistration {
        category_id: 627,
        type_id: Uuid::from_u128(0x451d459fbb17325cbb10d608f03f4c3f),
    },
    // ServiceCore.EndPointNetwork.UseInventoryItemFailedMessage
    ClassRegistration {
        category_id: 628,
        type_id: Uuid::from_u128(0xa9400bf7e1093e37afb67e99521f61e6),
    },
    // ServiceCore.EndPointNetwork.UseInventoryItemMessage
    ClassRegistration {
        category_id: 629,
        type_id: Uuid::from_u128(0xc182cb1874693390a0e78eba31d6e45d),
    },
    // ServiceCore.EndPointNetwork.VocationLearnSkillMessage
    ClassRegistration {
        category_id: 630,
        type_id: Uuid::from_u128(0x40b6b8a682c035ba99ec194bb8ac25f8),
    },
    // ServiceCore.EndPointNetwork.VocationLevelUpMessage
    ClassRegistration {
        category_id: 631,
        type_id: Uuid::from_u128(0x0c1b9a161e6f367d8ea997831f29ea25),
    },
    // ServiceCore.EndPointNetwork.VocationSkillListMessage
    ClassRegistration {
        category_id: 632,
        type_id: Uuid::from_u128(0x64a2137a4dd530a49de7b64e1751bd94),
    },
    // ServiceCore.EndPointNetwork.VocationTransformFinishedMessage
    ClassRegistration {
        category_id: 633,
        type_id: Uuid::from_u128(0x90cb3bf0a84b32d8987bcb9d0242cea3),
    },
    // ServiceCore.EndPointNetwork.VocationTransformMessage
    ClassRegistration {
        category_id: 634,
        type_id: Uuid::from_u128(0xee3e9401b0d437e290b72785855ef890),
    },
    // ServiceCore.EndPointNetwork.WaitTicketMessage
    ClassRegistration {
        category_id: 635,
        type_id: Uuid::from_u128(0xf7c5ea5695cf39e3ba1265e9795f360d),
    },
    // ServiceCore.EndPointNetwork.WhisperFilterListMessage
    ClassRegistration {
        category_id: 636,
        type_id: Uuid::from_u128(0x9d9c801a47003cd3889e99b210174432),
    },
    // ServiceCore.EndPointNetwork.UpdateWhisperFilterMessage
    ClassRegistration {
        category_id: 637,
        type_id: Uuid::from_u128(0xa7953e9f47e23ec39aacc9839f26427d),
    },
    // ServiceCore.EndPointNetwork.RequestWhisperMessage
    ClassRegistration {
        category_id: 638,
        type_id: Uuid::from_u128(0x5e3ee58f1ed53582b197aa2273d6b334),
    },
    // ServiceCore.EndPointNetwork.WhisperMessage
    ClassRegistration {
        category_id: 639,
        type_id: Uuid::from_u128(0xac053a13a83c322a9903e0f1f68d1287),
    },
    // ServiceCore.EndPointNetwork.WhisperFailMessage
    ClassRegistration {
        category_id: 640,
        type_id: Uuid::from_u128(0x905bd8c4246c302c9f46c9a7b8927a87),
    },
    // ServiceCore.EndPointNetwork._CheatCommandMessage
    ClassRegistration {
        category_id: 641,
        type_id: Uuid::from_u128(0x66db19b61e0430558035d1823eed3f27),
    },
    // ServiceCore.EndPointNetwork._CheatSetCafeStatusMessage
    ClassRegistration {
        category_id: 642,
        type_id: Uuid::from_u128(0x3b04d79d09e2324c902ee1896983b623),
    },
    // ServiceCore.EndPointNetwork._CheatSetLevelMessage
    ClassRegistration {
        category_id: 643,
        type_id: Uuid::from_u128(0xe6748394ee3d3b36a78da254cbf7f853),
    },
    // ServiceCore.EndPointNetwork._UpdatePlayState
    ClassRegistration {
        category_id: 644,
        type_id: Uuid::from_u128(0x62e666da21033f50b021aa8f0f0aa617),
    },
    // ServiceCore.EndPointNetwork.DSServiceInfoMessage
    ClassRegistration {
        category_id: 645,
        type_id: Uuid::from_u128(0x9161d54e87793000b79940ce62768972),
    },
    // ServiceCore.EndPointNetwork.QueryDSServiceInfoMessage
    ClassRegistration {
        category_id: 646,
        type_id: Uuid::from_u128(0x7d5d72d8b14a3972899434a894061e3a),
    },
    // ServiceCore.EndPointNetwork.Pvp.ArenaResultInfoMessage
    ClassRegistration {
        category_id: 647,
        type_id: Uuid::from_u128(0xc36c9bb6997d3d089a9aeddab5a6dadf),
    },
    // ServiceCore.EndPointNetwork.Pvp.PvpPenaltyMessage
    ClassRegistration {
        category_id: 648,
        type_id: Uuid::from_u128(0x6b85dc42733c366086459ed785d1b7e3),
    },
    // ServiceCore.EndPointNetwork.SetStoryHintCategoryMessage
    ClassRegistration {
        category_id: 649,
        type_id: Uuid::from_u128(0x8e059aabc691340b8aa7efb2aa9ca181),
    },
];
/// Recovered UserLoginMessage category for this source hash.
pub const USER_LOGIN_MESSAGE_CATEGORY: i32 = 478;
/// Recovered SyncFeatureMatrixMessage category for this source hash.
pub const SYNC_FEATURE_MATRIX_MESSAGE_CATEGORY: i32 = 185;
/// Recovered LoginFailMessage category for this source hash.
pub const LOGIN_FAIL_MESSAGE_CATEGORY: i32 = 484;
