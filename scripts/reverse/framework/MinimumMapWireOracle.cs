using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using Devcat.Core;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;
using ServiceCore.CharacterServiceOperations;
using MMOServer;
using HeroesChannelServer;
using HeroesChannelServer.Message;
using Utility;

class MinimumMapWireOracle {
    static int[] Body(Packet p) {
        return p.Array.Skip(p.Offset + p.BodyOffset).Take(p.Length).Select(b => (int)b).ToArray();
    }

    static uint FloatBits(float v) {
        return BitConverter.ToUInt32(BitConverter.GetBytes(v), 0);
    }

    static float FromBits(uint bits) {
        return BitConverter.ToSingle(BitConverter.GetBytes(bits), 0);
    }

    static object CaptureVector(Vector3D v) {
        return new {
            x_bits = FloatBits(v.X),
            y_bits = FloatBits(v.Y),
            z_bits = FloatBits(v.Z)
        };
    }

    static object CaptureAction(ActionSync a) {
        return new {
            position = CaptureVector(a.Position),
            velocity = CaptureVector(a.Velocity),
            yaw_bits = FloatBits(a.Yaw),
            sequence = (int)a.Sequence,
            action_state_index = (int)a.ActionStateIndex,
            start_time_bits = FloatBits(a.StartTime),
            state = a.State
        };
    }

    static object Capture(object v) {
        if (v == null) return null;
        if (v is float) return new { bits = FloatBits((float)v) };
        if (v is Vector3D) return CaptureVector((Vector3D)v);
        if (v is ActionSync) return CaptureAction((ActionSync)v);
        if (v.GetType().IsEnum) return Convert.ToInt32(v);
        if (v is string || v is int || v is long || v is short || v is bool || v is byte) return v;
        if (v is CharacterCommonInfoMessage) {
            var m = (CharacterCommonInfoMessage)v;
            long qid = (long)typeof(CharacterCommonInfoMessage)
                .GetField("queryID", BindingFlags.Instance | BindingFlags.NonPublic)
                .GetValue(m);
            return new {
                QueryID = qid,
                Info = Capture(m.Info)
            };
        }
        if (v is IDictionary) {
            var rows = new List<object>();
            foreach (DictionaryEntry d in (IDictionary)v) {
                rows.Add(new { key = Capture(d.Key), value = Capture(d.Value) });
            }
            return rows;
        }
        if (v is IEnumerable) {
            return ((IEnumerable)v).Cast<object>().Select(Capture).ToArray();
        }
        return v.GetType()
            .GetProperties(BindingFlags.Instance | BindingFlags.Public)
            .Where(p => p.CanRead)
            .ToDictionary(p => p.Name, p => Capture(p.GetValue(v, null)));
    }

    static CharacterSummary Summary(int seed, bool detailed) {
        var ctor = typeof(CharacterSummary).GetConstructors().Single();
        var args = ctor.GetParameters().Select((p, i) =>
            p.ParameterType == typeof(string) ? (object)(p.Name == "characterID" ? "Murael" : "") :
            p.ParameterType == typeof(int) ? (object)unchecked(seed + i * 17) :
            p.ParameterType == typeof(long) ? (object)(seed == int.MinValue ? long.MinValue : seed == int.MaxValue ? long.MaxValue : (long)seed + 1000L) :
            p.ParameterType == typeof(bool) ? (object)(((seed + i) & 1) == 0) :
            p.ParameterType.IsEnum ? Enum.ToObject(p.ParameterType, 1) : null
        ).ToArray();
        var c = (CharacterSummary)ctor.Invoke(args);
        if (detailed) {
            var x = c.Costume;
            x.CostumeTypeInfo.Add(0, 501);
            x.ColorInfo.Add(0, 0x112233);
            x.AvatarInfo.Add(0, false);
            x.AvatarHideInfo.Add(0, 0);
            x.PollutionInfo.Add(0, 12);
            x.EffectInfo.Add(0, 3);
            x.DecorationInfo.Add(0, 7);
            x.DecorationColorInfo.Add(0, 9);
            x.BodyShapeInfo.Add(1, FromBits(0x3f800000u));
            x.BodyShapeInfo.Add(2, FromBits(0x80000000u));
            x.Height = 50;
            x.Bust = -25;
            x.Shineness = 30;
        }
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

    sealed class TestCamera : ICamera {
        public long ID { get; set; }
        public long WatchingPartition { get; set; }
        public List<object> Packets = new List<object>();

        public void Update(MMOServer.IMessage message) {
            if (message == null) return;
            Packet pkt;
            if (message is PacketMessage) {
                pkt = ((PacketMessage)message).Data;
            } else if (message is MMOServer.Disappeared) {
                var d = (MMOServer.Disappeared)message;
                pkt = SerializeWriter.ToBinary(new ServiceCore.EndPointNetwork.Disappeared { ID = d.ID });
            } else {
                throw new InvalidOperationException("Unexpected MMOServer message type");
            }
            Packets.Add(new {
                category = pkt.CategoryId,
                body = Body(pkt)
            });
        }

        public void Update(IEnumerable<MMOServer.IMessage> messages) {
            foreach (var m in messages) Update(m);
        }

        public void Flush() {}
    }

    static void Main(string[] args) {
        if (args.Length != 1) {
            throw new InvalidOperationException("Expected MapPartition directory path");
        }
        string mapDir = args[0];

        var actions = new[] {
            default(ActionSync),
            new ActionSync {
                Position = new Vector3D { X = 123.5f, Y = -456.25f, Z = 0.0f },
                Velocity = new Vector3D { X = FromBits(0x80000000u), Y = 1.0f, Z = -2.0f },
                Yaw = 3.1415927f,
                Sequence = 1,
                ActionStateIndex = 2,
                StartTime = 0.5f,
                State = 7
            },
            new ActionSync {
                Position = new Vector3D { X = float.MinValue, Y = float.MaxValue, Z = FromBits(0x7fc01234u) },
                Velocity = new Vector3D { X = float.NegativeInfinity, Y = float.PositiveInfinity, Z = float.Epsilon },
                Yaw = -0.0f,
                Sequence = short.MinValue,
                ActionStateIndex = short.MaxValue,
                StartTime = -100.75f,
                State = int.MinValue
            }
        };

        var wireCases = new List<object>();

        AddCases(
            "notice_game_environment",
            345,
            new Guid("97636de8-6ff1-3d4c-8c53-b8f1da851df4"),
            new NoticeGameEnvironmentMessage[] {
                null,
                new NoticeGameEnvironmentMessage(0, false),
                new NoticeGameEnvironmentMessage(1, true),
                new NoticeGameEnvironmentMessage(int.MinValue, false),
                new NoticeGameEnvironmentMessage(int.MaxValue, true)
            },
            wireCases
        );

        AddCases(
            "identify",
            363,
            new Guid("428dee64-190d-3cf6-8450-3e511769c608"),
            new Identify[] {
                null,
                new Identify { ID = 0L, Key = 0 },
                new Identify { ID = 1L, Key = 123456789 },
                new Identify { ID = long.MinValue, Key = int.MinValue },
                new Identify { ID = long.MaxValue, Key = int.MaxValue }
            },
            wireCases
        );

        AddCases(
            "enter_channel",
            364,
            new Guid("79ce9a53-8956-3645-a67b-f69681ea7a35"),
            new EnterChannel[] { null }.Concat(
                actions.Select((a, i) => new EnterChannel {
                    ChannelID = i == 0 ? 1L : i == 1 ? 5L : long.MaxValue,
                    PartitionID = i == 0 ? 6479215318592913461L : i == 1 ? 0L : long.MinValue,
                    Action = a
                })
            ),
            wireCases
        );

        AddCases(
            "channel_server_address",
            365,
            new Guid("85bf1995-487c-3e7e-8603-e88d75d21b46"),
            new ChannelServerAddress[] {
                null,
                new ChannelServerAddress { ChannelID = 1L, Address = "127.0.0.1", Port = 27003, Key = 987654321 },
                new ChannelServerAddress { ChannelID = -1L, Address = "", Port = 0, Key = 0 },
                new ChannelServerAddress { ChannelID = -2L, Address = null, Port = int.MaxValue, Key = int.MinValue }
            },
            wireCases
        );

        AddCases(
            "identify_failed",
            366,
            new Guid("42fa5d33-391e-320b-85db-0c61e1b7f6ba"),
            new IdentifyFailed[] {
                null,
                new IdentifyFailed("GameUI_Heroes_Channel_Error_IdentifyFail"),
                new IdentifyFailed("GameUI_Heroes_Channel_Error_KeyMismatch"),
                new IdentifyFailed(""),
                new IdentifyFailed(null)
            },
            wireCases
        );

        AddCases(
            "move_partition",
            368,
            new Guid("d3cf4e65-f3a6-37f4-aae6-d8819a02e728"),
            new ServiceCore.EndPointNetwork.MovePartition[] {
                null,
                new ServiceCore.EndPointNetwork.MovePartition { TargetPartitionID = 0L },
                new ServiceCore.EndPointNetwork.MovePartition { TargetPartitionID = 6479215318592913461L },
                new ServiceCore.EndPointNetwork.MovePartition { TargetPartitionID = long.MinValue },
                new ServiceCore.EndPointNetwork.MovePartition { TargetPartitionID = long.MaxValue }
            },
            wireCases
        );

        AddCases(
            "update_action",
            369,
            new Guid("bb98e908-edbe-32d5-89c2-eb38d6508cad"),
            new UpdateAction[] { null }.Concat(actions.Select(a => new UpdateAction { Data = a })),
            wireCases
        );

        AddCases(
            "disappeared",
            372,
            new Guid("4f13a705-3f02-37cb-b869-ebfbf3852ce2"),
            new ServiceCore.EndPointNetwork.Disappeared[] {
                null,
                new ServiceCore.EndPointNetwork.Disappeared { ID = 0L },
                new ServiceCore.EndPointNetwork.Disappeared { ID = 1L },
                new ServiceCore.EndPointNetwork.Disappeared { ID = long.MinValue },
                new ServiceCore.EndPointNetwork.Disappeared { ID = long.MaxValue }
            },
            wireCases
        );

        AddCases(
            "notify_look",
            373,
            new Guid("711f48ae-c601-344c-bb5a-9320b001f809"),
            new NotifyLook[] {
                null,
                new NotifyLook { ID = 1L, Look = null },
                new NotifyLook { ID = 1L, Look = Summary(0, false) },
                new NotifyLook { ID = 1001L, Look = Summary(7, true) }
            },
            wireCases
        );

        AddCases(
            "notify_action",
            374,
            new Guid("86b97474-ac74-3722-bda1-a8ba43968733"),
            new NotifyAction[] { null }.Concat(
                actions.Select((a, i) => new NotifyAction {
                    ID = i == 0 ? 1L : i == 1 ? 1001L : long.MinValue,
                    Action = a
                })
            ),
            wireCases
        );

        AddCases(
            "select_character",
            481,
            new Guid("1f34c672-5581-353f-9944-d5ab72e808a5"),
            new SelectCharacterMessage[] {
                null,
                new SelectCharacterMessage(0),
                new SelectCharacterMessage(1),
                new SelectCharacterMessage(-1),
                new SelectCharacterMessage(int.MinValue),
                new SelectCharacterMessage(int.MaxValue)
            },
            wireCases
        );

        AddCases(
            "login_ok",
            483,
            new Guid("e301ee14-10c2-3886-8533-fbba9ef1cbe1"),
            new LoginOkMessage[] {
                null,
                new LoginOkMessage {
                    RegionCode = 0,
                    Time = 1710000000,
                    Limited = 0,
                    FacebookToken = "",
                    UserCareType = -1,
                    UserCareNextState = -1,
                    MapStateInfo = 0
                },
                new LoginOkMessage {
                    RegionCode = int.MaxValue,
                    Time = int.MinValue,
                    Limited = 2,
                    FacebookToken = null,
                    UserCareType = 3,
                    UserCareNextState = 4,
                    MapStateInfo = 255
                }
            },
            wireCases
        );

        var commonInfo1 = new CharacterCommonInfoMessage(Summary(0, false));
        var commonInfo2 = new CharacterCommonInfoMessage(Summary(7, true)) { QueryID = 42L };
        var commonInfo3 = new CharacterCommonInfoMessage(null) { QueryID = long.MinValue };
        AddCases(
            "character_common_info",
            609,
            new Guid("9756914f-6753-3228-b0b8-38b30dc1a612"),
            new CharacterCommonInfoMessage[] { null, commonInfo1, commonInfo2, commonInfo3 },
            wireCases
        );

        var fullStats = new Dictionary<string, int>();
        string[] statNames = {
            "ATK", "MATK", "DEF", "STR", "DEX", "INT", "WILL", "LUCK", "HP", "STAMINA",
            "HEAVY_LOAD", "LOAD_PENALTY", "MEDIUM_LOAD", "TOTAL_LOAD", "ATK_Speed",
            "ATK_Absolute", "Critical", "CritFactor", "Res_Critical", "Balance",
            "PVP_ATK", "PVP_MATK", "PVP_DEF", "TOWN_SPEED", "SKILL_RANK_SUM"
        };
        for (int i = 0; i < statNames.Length; i++) {
            fullStats[statNames[i]] = (i + 1) * 10;
        }
        AddCases(
            "update_stat",
            619,
            new Guid("a1407a79-5353-3455-9a2c-859f8825c200"),
            new UpdateStatMessage[] {
                null,
                new UpdateStatMessage(null),
                new UpdateStatMessage(new Dictionary<string, int>()),
                new UpdateStatMessage(fullStats),
                new UpdateStatMessage(new Dictionary<string, int> { { "STR", int.MaxValue }, { "LOAD_PENALTY", int.MinValue } })
            },
            wireCases
        );

        var increase = new List<KeyValuePair<string, int>>();
        foreach (string name in statNames) { increase.Add(new KeyValuePair<string, int>(name, name == "HP" ? 28 : name == "STR" ? 7 : 0)); }
        AddCases(
            "level_up",
            476,
            new Guid("23314733-9565-36b4-a382-8042ea30debe"),
            new LevelUpMessage[] {
                null,
                new LevelUpMessage(2, increase),
                new LevelUpMessage(int.MaxValue, new List<KeyValuePair<string, int>>()),
                new LevelUpMessage(-1, new List<KeyValuePair<string, int>> { new KeyValuePair<string, int>("STR", int.MinValue) })
            },
            wireCases
        );

        long[] testTicks = {
            621355968000000000L, // 1970-01-01T00:00:00Z
            621355967990000000L, // -1s
            621355968009999999L, // +0.9999999s
            638448480000000000L, // 2024-03-01T00:00:00Z
            639280320000000000L, // 2026-10-19T00:00:00Z
            642816214470000000L, // 2038-01-19T03:14:07Z (i32::MAX)
            642816214480000000L  // 2038-01-19T03:14:08Z (overflow wrap)
        };
        var epochCases = testTicks.Select(t => new {
            ticks = t.ToString(),
            seconds = new DateTime(t, DateTimeKind.Utc).ToInt32()
        }).ToArray();

        // MapFile + Map + Channel topology
        string[] files = Directory.GetFiles(mapDir, "*.dat", SearchOption.AllDirectories)
            .OrderBy(p => Path.GetFileName(p), StringComparer.Ordinal)
            .ToArray();
        var channel = new Channel();
        var mapEntries = new List<object>();
        var allPartitionIds = new List<long> { 0L };

        foreach (string file in files) {
            string bsp = Path.GetFileNameWithoutExtension(file);
            var mf = new MapFile(file);
            if (!mf.Load()) {
                throw new InvalidOperationException("Failed to load MapFile: " + file);
            }
            var map = new Map(bsp, file);
            bool built = map.Build(channel);
            foreach (long pid in mf.Partitions) {
                if (!allPartitionIds.Contains(pid)) {
                    allPartitionIds.Add(pid);
                }
            }
            mapEntries.Add(new {
                bsp,
                file_length = new FileInfo(file).Length,
                built,
                partitions = mf.Partitions.Select(p => p.ToString()).ToArray(),
                links = mf.VisibleLinks.Select(kv => new { lhs = kv.Key.ToString(), rhs = kv.Value.ToString() }).ToArray()
            });
        }
        channel.ConfirmRegions(1);

        var neighborEntries = allPartitionIds.Select(pid => new {
            pid = pid.ToString(),
            visible = channel.GetNeighborPartitions(pid).Select(x => x.ToString()).ToArray()
        }).ToArray();

        // Multi-player Channel + Look + Kinetics + MovePartition simulation
        var cam1 = new TestCamera { ID = 1001L };
        var cam2 = new TestCamera { ID = 1002L };
        var look1 = Summary(1, false);
        var look2 = Summary(2, true);
        long pColhen1 = 6479215318592913461L;
        long pColhen2 = 6479215318592913462L;
        long pColhen3 = 6479215318592913463L;
        long pOtherRegion = allPartitionIds.First(p => p != 0L && !channel.GetNeighborPartitions(pColhen1).Contains(p));

        var steps = new List<object>();
        Action<string, Action> runStep = (name, act) => {
            cam1.Packets.Clear();
            cam2.Packets.Clear();
            act();
            steps.Add(new {
                step = name,
                cam1 = cam1.Packets.ToArray(),
                cam2 = cam2.Packets.ToArray(),
                cam1_watching = cam1.WatchingPartition.ToString(),
                cam2_watching = cam2.WatchingPartition.ToString()
            });
        };

        runStep("p1_enter_colhen1", () => {
            channel.AddComponent(channel.MakeLocation(1001L, 1, pColhen1));
            channel.AddComponent(new Look(1001L, look1));
            channel.AddComponent(new Kinetics(1001L, actions[0]));
            channel.AddObserver(pColhen1, cam1);
        });

        runStep("p2_enter_colhen2", () => {
            channel.AddComponent(channel.MakeLocation(1002L, 1, pColhen2));
            channel.AddComponent(new Look(1002L, look2));
            channel.AddComponent(new Kinetics(1002L, actions[1]));
            channel.AddObserver(pColhen2, cam2);
        });

        runStep("p1_action", () => {
            channel.ApplyModifier(new KineticsChanged(1001L, actions[1]));
        });

        runStep("p2_move_colhen3", () => {
            channel.ApplyModifier(channel.MakeMove(1002L, pColhen3));
            channel.MoveObserver(pColhen3, cam2);
        });

        runStep("p2_move_far_partition", () => {
            channel.ApplyModifier(channel.MakeMove(1002L, pOtherRegion));
            channel.MoveObserver(pOtherRegion, cam2);
        });

        runStep("p2_move_back_colhen1", () => {
            channel.ApplyModifier(channel.MakeMove(1002L, pColhen1));
            channel.MoveObserver(pColhen1, cam2);
        });

        runStep("p1_leave", () => {
            channel.RemoveObserver(cam1);
            foreach (var c in channel.Space.FindByID(1001L).ToList()) {
                channel.RemoveComponent(c);
            }
        });

        runStep("p2_leave", () => {
            channel.RemoveObserver(cam2);
            foreach (var c in channel.Space.FindByID(1002L).ToList()) {
                channel.RemoveComponent(c);
            }
        });

        var output = new {
            architecture = IntPtr.Size == 4 ? "x86" : "x64",
            wire_cases = wireCases,
            epoch_cases = epochCases,
            maps = mapEntries,
            neighbors = neighborEntries,
            channel_steps = steps
        };
        Console.WriteLine(new JavaScriptSerializer { MaxJsonLength = 32 * 1024 * 1024 }.Serialize(output));
    }
}

namespace Devcat.Core.Memory {
    public static class ChunkSegment<T> {
        public static ArraySegment<T> Get(int count) {
            return new ArraySegment<T>(new T[count]);
        }
    }
}
