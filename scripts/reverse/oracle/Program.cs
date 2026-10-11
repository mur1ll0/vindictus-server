using System.Text.Json;
using Devcat.Core.Net.Message;

var integers = new List<object>();
var values = new List<int> { 0, 1, 126, 127, 128, 2047, 2048, 65535, 65536, 2097151, 2097152, 67108863, 67108864, int.MaxValue, int.MinValue, -1, -2 };
var random = new Random(169);
for (int i = 0; i < 128; i++) values.Add((int)random.NextInt64(int.MinValue, (long)int.MaxValue + 1));
foreach (int value in values) {
    byte[] buffer = new byte[6];
    int count = Packet.GetBytes(value, buffer, 0);
    if (Packet.GetInt32(buffer, 0) != value) throw new Exception("C# round trip failed");
    integers.Add(new { value, bytes = buffer.Take(count).Select(x => (int)x).ToArray() });
}
var frames = new List<object>();
foreach (int length in new[] { 0, 1, 127, 128, 2047, 2048 }) {
    var packet = new Packet(length, -1234567);
    packet.InstanceId = -0x102030405060708L;
    var body = Enumerable.Range(0, length).Select(x => (byte)(x % 251)).ToArray();
    Array.Copy(body, 0, packet.Bytes.Array, packet.BodyOffset, body.Length);
    frames.Add(new { instance_id = packet.InstanceId, category_id = packet.CategoryId,
        body = body.Select(x => (int)x).ToArray(), bytes = packet.Bytes.Array.Select(x => (int)x).ToArray() });
}
var isaac = new List<object>();
foreach (int seed in new[] { 0, 1, 169, int.MaxValue, int.MinValue, -1 }) {
    var generator = new Isaac(seed);
    isaac.Add(new { seed, words = Enumerable.Range(0, 600).Select(_ => generator.Next()).ToArray() });
}
List<object> MakeTransport(bool heroes) {
var transport = new List<object>();
foreach (int randomSeed in new[] { 0, 1, 169, 2147483647 }) {
    (heroes ? typeof(ServiceCore.CryptoTransformHeroes) : typeof(CryptoTransform)).GetField("rand", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Static)!.SetValue(null, new Random(randomSeed));
    Devcat.Core.Net.ICryptoTransform sender = heroes ? new ServiceCore.CryptoTransformHeroes() : new CryptoTransform();
    var receiver = new MessageAnalyzer { CryptoTransform = heroes ? new ServiceCore.CryptoTransformHeroes() : new CryptoTransform() };
    var wire = new List<byte>();
    var expectedFrames = new List<object>();
    long salt = 0;
    int frameIndex = 0;
    foreach (int length in new[] { 0, 1, 127, 128, 2048 }) {
        var packet = new Packet(length, new[] { 0, 127, 128, -1, int.MaxValue }[frameIndex]);
        long instanceId = frameIndex == 0 ? 998877 : -99887766L - frameIndex;
        packet.InstanceId = instanceId;
        var body = Enumerable.Range(0, length).Select(x => (byte)(x % 251)).ToArray();
        Array.Copy(body, 0, packet.Bytes.Array, packet.BodyOffset, body.Length);
        int category = packet.CategoryId;
        packet.Encrypt(sender);
        if (frameIndex == 0) salt = packet.InstanceId;
        wire.AddRange(packet.Bytes);
        expectedFrames.Add(new { instance_id = frameIndex == 0 ? 0 : instanceId, original_instance_id = instanceId, category_id = category, body = body.Select(x => (int)x).ToArray() });
        frameIndex++;
    }
    var decoded = new List<object>();
    byte[] wireBytes = wire.ToArray();
    for (int offset = 0; offset < wireBytes.Length; offset += 7) {
        receiver.Add(new ArraySegment<byte>(wireBytes, offset, Math.Min(7, wireBytes.Length - offset)));
        foreach (var segment in receiver) {
            var packet = new Packet(segment);
            decoded.Add(new { instance_id = packet.InstanceId, category_id = packet.CategoryId, body = packet.Array.Skip(packet.Offset + packet.BodyOffset).Take(packet.Length).Select(x => (int)x).ToArray() });
        }
    }
    if (decoded.Count != expectedFrames.Count) throw new Exception("Incomplete recovered transport");
    transport.Add(new { salt, frames = expectedFrames, wire = wireBytes.Select(x => (int)x).ToArray(), decoded });
}
return transport;
}
var transport = MakeTransport(false);
var heroes_transport = MakeTransport(true);
var heroes_isaac = new List<object>();
foreach (uint seed in new uint[] { 0, 1, 16, 63, 64, 169, int.MaxValue, uint.MaxValue }) {
    var generator = new ServiceCore.Isaac(seed);
    heroes_isaac.Add(new { seed, words = Enumerable.Range(0, 600).Select(_ => generator.Next()).ToArray() });
}
// Category 1 is an oracle-only registration, not a recovered login opcode.
ClassInfo<ServiceCore.EndPointNetwork.UserLoginMessage>.CategoryId = 1;
var login = new List<object>();
foreach (bool populated in new[] { false, true }) {
    var message = new ServiceCore.EndPointNetwork.UserLoginMessage(populated ? "estudante-é\u03bb" : null);
    void Set(string property, object value) => message.GetType().GetProperty(property)!.SetValue(message, value);
    Set("LocalAddress", populated ? 0xC0A80001u : 0u);
    Set("hwID", populated ? "hardware-test" : null);
    Set("MachineID", populated ? Enumerable.Range(0, 16).Select(x => (byte)x).ToArray() : null);
    Set("GameRoomClient", populated ? -17 : 0);
    Set("IsCharacterSelectSkipped", populated);
    Set("NexonID", populated ? "Estudante" : "");
    Set("UpToDateInfo", populated ? new string('a', 128) : null);
    Set("CheckSum", populated ? long.MinValue + 169 : 0L);
    var packet = SerializeWriter.ToBinary(message);
    login.Add(new { value = message, body = packet.Array.Skip(packet.Offset + packet.BodyOffset).Take(packet.Length).Select(x => (int)x).ToArray() });
}
var nullLogin = SerializeWriter.ToBinary<ServiceCore.EndPointNetwork.UserLoginMessage>(null);
login.Add(new { value = (object)null, body = nullLogin.Array.Skip(nullLogin.Offset + nullLogin.BodyOffset).Take(nullLogin.Length).Select(x => (int)x).ToArray() });
Console.WriteLine(JsonSerializer.Serialize(new { oracle = "Recovered Core.dll Packet.cs, compiled unchanged with memory allocation shim", source_sha256 = "8942a0c26a9abf4c5f0d1f7ffff41452a3714d099e3b76012cebdee97293c2f0", integers, frames, isaac, transport, login, heroes_transport, heroes_isaac }, new JsonSerializerOptions { WriteIndented = true }));

namespace Devcat.Core.Memory {
    public static class ChunkSegment<T> {
        public static ArraySegment<T> Get(int count) => new(new T[count]);
    }
}
