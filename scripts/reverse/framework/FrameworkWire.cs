using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.IO;
using System.Security.Cryptography;
using System.Runtime.InteropServices;
using System.Web.Script.Serialization;
using Devcat.Core.Net.Message;
// Field shape and explicit GUID recovered from MessageHandlerFactory.TypeConverter.
// The fixture DTO copies that private one-field layout; the serializer sources are unchanged.
[Serializable, Guid("ffffffff-ffff-ffff-ffff-ffffffffffff")]
struct ConverterFixture {
    public IDictionary<int, Type> Dictionary { get; private set; }
    public ConverterFixture(IDictionary<int, Type> value) { Dictionary = value; }
}
[Serializable, Guid("47f6f00c-0bd3-3059-afad-84a872ed7399")]
sealed class LoginTypeFixture {}
[Serializable, Guid("966ed54f-4129-37ad-a922-66fa4360f61a")]
sealed class FeatureTypeFixture {}
class FrameworkWire {
    static void Main(string[] args) {
        if(args.Length != 1) throw new ArgumentException("Original ServiceCore.dll required for complete converter fixture");
        string original = Path.GetFullPath(args[0]);
        string sourceHash;
        using(var sha = SHA256.Create()) using(var file = File.OpenRead(original)) { sourceHash = BitConverter.ToString(sha.ComputeHash(file)).Replace("-", "").ToLowerInvariant(); }
        if(sourceHash != "be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683") throw new InvalidDataException("Unexpected ServiceCore hash");
        AppDomain.CurrentDomain.ReflectionOnlyAssemblyResolve += delegate(object sender, ResolveEventArgs eventArgs) {
            string dependency = Path.Combine(Path.GetDirectoryName(original), new AssemblyName(eventArgs.Name).Name + ".dll");
            return File.Exists(dependency) ? Assembly.ReflectionOnlyLoadFrom(dependency) : Assembly.ReflectionOnlyLoad(eventArgs.Name);
        };
        var source = Assembly.ReflectionOnlyLoadFrom(original);
        if(!source.ReflectionOnly) throw new InvalidOperationException("Original must remain ReflectionOnly");
        var messageInterface = source.GetType("ServiceCore.EndPointNetwork.IMessage", true);
        var registered = new Dictionary<int, Type>();
        int category = 1;
        foreach(var type in source.GetTypes()) {
            if(type.Namespace != null && type.Namespace.StartsWith("ServiceCore.EndPointNetwork", StringComparison.Ordinal) && !type.IsInterface && !type.IsAbstract && messageInterface.IsAssignableFrom(type) && type.IsSealed) registered.Add(category++, type);
        }
        var fixtures = new List<object>();
        foreach(var count in new[] { -1, 0, 2, 128, registered.Count }) {
            IDictionary<int, Type> dictionary = count < 0 ? null : new Dictionary<int, Type>();
            if(count == 2) { dictionary.Add(478, typeof(LoginTypeFixture)); dictionary.Add(185, typeof(FeatureTypeFixture)); }
            if(count == 128) for(int i = 0; i < count; i++) dictionary.Add(i + 1, typeof(LoginTypeFixture));
            if(count == registered.Count) dictionary = registered;
            var packet = SerializeWriter.ToBinary(new ConverterFixture(dictionary));
            var entries = dictionary == null ? null : dictionary.Select(pair => new { category = pair.Key, guid = pair.Value.GUID.ToString() }).ToArray();
            fixtures.Add(new { entries = entries, category = packet.CategoryId, body = packet.Array.Skip(packet.Offset + packet.BodyOffset).Take(packet.Length).Select(b => (int)b).ToArray() });
        }
        ClassInfo<ServiceCore.EndPointNetwork.SyncFeatureMatrixMessage>.CategoryId = 185;
        ClassInfo<ServiceCore.EndPointNetwork.LoginFailMessage>.CategoryId = 484;
        var failures = new List<object>();
        foreach(var reason in new[] { -1, 0, 1, 13, 24, int.MinValue, int.MaxValue }) {
            foreach(var banned in new string[] { null, "", "Estudo-É-λ" }) {
                var message = new ServiceCore.EndPointNetwork.LoginFailMessage { Reason = reason, BannedReason = banned };
                var packet = SerializeWriter.ToBinary(message);
                failures.Add(new { reason = reason, banned_reason = banned, category = packet.CategoryId, body = packet.Array.Skip(packet.Offset + packet.BodyOffset).Take(packet.Length).Select(b => (int)b).ToArray() });
            }
        }
        var features = new List<object>();
        foreach(var count in new[] { -2, -1, 0, 2, 128 }) {
            Dictionary<string, string> dictionary = count < 0 ? null : new Dictionary<string, string>();
            if(count == 2) { dictionary.Add("NGM_disable", "1"); dictionary.Add("Português-\u03bb", null); }
            if(count == 128) for(int i = 0; i < count; i++) dictionary.Add("Feature_"+i, i % 2 == 0 ? "" : "enabled");
            var message = count == -2 ? null : new ServiceCore.EndPointNetwork.SyncFeatureMatrixMessage(dictionary);
            var packet = SerializeWriter.ToBinary(message);
            features.Add(new { message_is_null = message == null, entries = dictionary == null ? null : dictionary.Select(pair => new { key = pair.Key, value = pair.Value }).ToArray(), category = packet.CategoryId, body = packet.Array.Skip(packet.Offset + packet.BodyOffset).Take(packet.Length).Select(b => (int)b).ToArray() });
        }
        var initialFeatures = new Dictionary<string, string>();
        initialFeatures.Add("NGM_disable", "1"); initialFeatures.Add("TestUnicode", "É-\u03bb");
        var initialConverter = SerializeWriter.ToBinary(new ConverterFixture(registered));
        var initialMatrix = SerializeWriter.ToBinary(new ServiceCore.EndPointNetwork.SyncFeatureMatrixMessage(initialFeatures));
        typeof(ServiceCore.CryptoTransformHeroes).GetField("rand", BindingFlags.NonPublic | BindingFlags.Static).SetValue(null, new Random(169));
        var cipher = new ServiceCore.CryptoTransformHeroes();
        initialConverter.Encrypt(cipher);
        long salt = initialConverter.InstanceId;
        initialMatrix.Encrypt(cipher);
        var initialWire = initialConverter.Bytes.Concat(initialMatrix.Bytes).Select(b => (int)b).ToArray();
        // A third packet proves that the returned Rust sender retains stream state.
        var followup = SerializeWriter.ToBinary(new ServiceCore.EndPointNetwork.SyncFeatureMatrixMessage(initialFeatures));
        followup.InstanceId = 123456789;
        followup.Encrypt(cipher);
        var bootstrap = new { salt = salt, entries = initialFeatures.Select(pair => new { key = pair.Key, value = pair.Value }).ToArray(), wire = initialWire, followup_wire = followup.Bytes.Select(b => (int)b).ToArray() };
        Console.WriteLine(new JavaScriptSerializer().Serialize(new { runtime = Environment.Version.ToString(), source_sha256 = sourceHash, method = "Unchanged recovered serializer on .NET Framework BCL; fixture struct matches TypeConverter private field shape and explicit GUID", converters = fixtures, features = features, failures = failures, bootstrap = bootstrap }));
    }
}
namespace Devcat.Core.Memory {
    public static class ChunkSegment<T> {
        public static ArraySegment<T> Get(int count) { return new ArraySegment<T>(new T[count]); }
    }
}
