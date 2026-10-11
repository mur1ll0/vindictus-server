// Reviewable .NET Framework metadata oracle. Original assemblies are loaded only
// into ReflectionOnly context: no constructors, methods or attribute instances run.
using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Security.Cryptography;
using System.Web.Script.Serialization;
class FrameworkTypes {
    static string directory;
    static Assembly Resolve(object sender, ResolveEventArgs args) {
        string path = Path.Combine(directory, new AssemblyName(args.Name).Name + ".dll");
        return File.Exists(path) ? Assembly.ReflectionOnlyLoadFrom(path) : Assembly.ReflectionOnlyLoad(args.Name);
    }
    static int Main(string[] args) {
        try {
            if(args.Length != 1) throw new ArgumentException("Expected original ServiceCore.dll path");
            string path = Path.GetFullPath(args[0]);
            directory = Path.GetDirectoryName(path);
            string hash;
            using(var sha = SHA256.Create()) using(var file = File.OpenRead(path)) {
                hash = BitConverter.ToString(sha.ComputeHash(file)).Replace("-", "").ToLowerInvariant();
            }
            if(hash != "be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683") throw new InvalidDataException("Unexpected ServiceCore source hash");
            AppDomain.CurrentDomain.ReflectionOnlyAssemblyResolve += Resolve;
            Assembly assembly = Assembly.ReflectionOnlyLoadFrom(path);
            if(!assembly.ReflectionOnly) throw new InvalidOperationException("Original assembly was not loaded ReflectionOnly");
            Type messageInterface = assembly.GetType("ServiceCore.EndPointNetwork.IMessage", true);
            var rows = new List<object>();
            int category = 1;
            foreach(Type type in assembly.GetTypes()) {
                if(type.Namespace == null || !type.Namespace.StartsWith("ServiceCore.EndPointNetwork", StringComparison.Ordinal) || type.IsInterface || type.IsAbstract || !messageInterface.IsAssignableFrom(type)) continue;
                var fields = new List<object>();
                foreach(var field in type.GetFields(BindingFlags.DeclaredOnly | BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic)) {
                    if(field.IsInitOnly || field.IsLiteral || field.IsNotSerialized) continue;
                    fields.Add(new { name = field.Name, type = field.FieldType.FullName, token = field.MetadataToken });
                }
                rows.Add(new { name = type.FullName, guid = type.GUID.ToString(), category = type.IsSealed ? category++ : 0, token = type.MetadataToken, fields = fields });
            }
            var output = new { method = "Assembly.ReflectionOnlyLoadFrom on original metadata; no original assembly execution", source_sha256 = hash, runtime = Environment.Version.ToString(), process_bits = IntPtr.Size * 8, assembly_name = assembly.FullName, messages = rows };
            var serializer = new JavaScriptSerializer { MaxJsonLength = int.MaxValue };
            Console.WriteLine(serializer.Serialize(output));
            return 0;
        } catch(ReflectionTypeLoadException error) {
            foreach(var detail in error.LoaderExceptions) Console.Error.WriteLine(detail.Message);
            return 2;
        } catch(Exception error) {
            Console.Error.WriteLine(error.ToString());
            return 1;
        }
    }
}
