using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
using ServiceCore.EndPointNetwork;

class LoginEnvelopeOracle {
    static int[] Body(Packet packet) { return packet.Array.Skip(packet.Offset + packet.BodyOffset).Take(packet.Length).Select(b => (int)b).ToArray(); }
    static void Set(UserLoginMessage message, string name, object value) { typeof(UserLoginMessage).GetProperty(name).SetValue(message, value, null); }
    static void Main() {
        var expectedGuid = new Guid("47f6f00c-0bd3-3059-afad-84a872ed7399");
        if (typeof(UserLoginMessage).GUID != expectedGuid) throw new InvalidOperationException("Pinned original type identity mismatch");
        var messages = new List<UserLoginMessage> { null };
        for (int i = 0; i < 12; i++) {
            var message = new UserLoginMessage(i == 0 ? null : i == 1 ? "" : i == 2 ? "lab-É-λ" : new string('a', new[] {24,127,128,2048}[i % 4]));
            Set(message, "LocalAddress", i % 2 == 0 ? 0u : uint.MaxValue);
            Set(message, "hwID", i % 3 == 0 ? null : i % 3 == 1 ? "" : "lab-hardware");
            Set(message, "MachineID", i % 3 == 0 ? null : Enumerable.Range(0, i % 3 == 1 ? 0 : 16).Select(n => (byte)n).ToArray());
            Set(message, "GameRoomClient", new[] {0,-1,int.MinValue,int.MaxValue}[i % 4]);
            Set(message, "IsCharacterSelectSkipped", i % 2 != 0);
            Set(message, "NexonID", i % 3 == 0 ? null : i % 3 == 1 ? "" : "LabUser");
            Set(message, "UpToDateInfo", i % 3 == 0 ? null : i % 3 == 1 ? "" : "synthetic-version");
            Set(message, "CheckSum", new[] {0L,-1L,long.MinValue,long.MaxValue}[i % 4]);
            messages.Add(message);
        }
        var generic = messages.Select(message => SerializeWriter.ToBinary<UserLoginMessage>(message)).ToArray();
        ClassInfo<UserLoginMessage>.CategoryId = 478;
        var cases = new List<object>();
        for (int i = 0; i < messages.Count; i++) {
            var typed = SerializeWriter.ToBinary<UserLoginMessage>(messages[i]);
            foreach (string mode in new[] {"unregistered-generic", "registered-object", "registered-typed"}) {
                var packet = mode == "unregistered-generic" ? generic[i] : mode == "registered-object" ? SerializeWriter.ToBinary((object)messages[i]) : typed;
                UserLoginMessage decoded;
                SerializeReader.FromBinary<UserLoginMessage>(packet, out decoded);
                var roundtrip = SerializeWriter.ToBinary<UserLoginMessage>(decoded);
                if (!Body(roundtrip).SequenceEqual(Body(typed))) throw new InvalidOperationException("Recovered reader roundtrip differs");
                var body = Body(packet);
                var typedBody = Body(typed);
                typeof(ServiceCore.CryptoTransformHeroes).GetField("rand", BindingFlags.NonPublic | BindingFlags.Static).SetValue(null, new Random(169));
                packet.Encrypt(new ServiceCore.CryptoTransformHeroes());
                cases.Add(new { id = i, mode, category = mode == "registered-typed" ? 478 : 0, body, typed_body = typedBody, wire = packet.Bytes.Select(b => (int)b).ToArray(), message_is_null = messages[i] == null });
            }
        }
        Console.WriteLine(new JavaScriptSerializer { MaxJsonLength = 4*1024*1024 }.Serialize(new {
            architecture = IntPtr.Size == 4 ? "x86" : "x64", runtime = Environment.Version.ToString(),
            login_guid = expectedGuid.ToString(), object_guid = typeof(object).GUID.ToString(), cases
        }));
    }
}
namespace Devcat.Core.Memory {
    public static class ChunkSegment<T> { public static ArraySegment<T> Get(int count) { return new ArraySegment<T>(new T[count]); } }
}
