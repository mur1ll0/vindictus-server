using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.Metadata.Ecma335;
using System.Reflection.PortableExecutable;
using System.Security.Cryptography;
using System.Text.Json;
if (args.Length != 1) throw new ArgumentException("Provide an original ServiceCore.dll path");
using var stream = File.OpenRead(args[0]);
var hash = Convert.ToHexString(SHA256.HashData(stream)).ToLowerInvariant();
stream.Position = 0;
using var pe = new PEReader(stream);
var reader = pe.GetMetadataReader();
string Name(EntityHandle handle) {
    if (handle.Kind == HandleKind.TypeDefinition) {
        var type = reader.GetTypeDefinition((TypeDefinitionHandle)handle);
        string ns = reader.GetString(type.Namespace), name = reader.GetString(type.Name);
        var parent = type.GetDeclaringType();
        if (!parent.IsNil) return Name(parent) + "+" + name;
        return string.IsNullOrEmpty(ns) ? name : ns + "." + name;
    }
    if (handle.Kind == HandleKind.TypeReference) {
        var type = reader.GetTypeReference((TypeReferenceHandle)handle);
        return reader.GetString(type.Namespace) + "." + reader.GetString(type.Name);
    }
    return "<" + handle.Kind + ">";
}
var unresolved = new HashSet<string>();
bool IsMessage(EntityHandle handle, HashSet<EntityHandle> seen) {
    if (handle.IsNil || !seen.Add(handle)) return false;
    if (Name(handle) == "ServiceCore.EndPointNetwork.IMessage") return true;
    if (handle.Kind == HandleKind.TypeDefinition) {
        var type = reader.GetTypeDefinition((TypeDefinitionHandle)handle);
        foreach (var iface in type.GetInterfaceImplementations())
            if (IsMessage(reader.GetInterfaceImplementation(iface).Interface, seen)) return true;
        return IsMessage(type.BaseType, seen);
    }
    if (handle.Kind == HandleKind.TypeSpecification) unresolved.Add(Name(handle));
    return false;
}
var rows = new List<object>();
int nextCategory = 1;
foreach (var handle in reader.TypeDefinitions) {
    var type = reader.GetTypeDefinition(handle);
    string name = Name(handle);
    if (!name.StartsWith("ServiceCore.EndPointNetwork.", StringComparison.Ordinal)) continue;
    if ((type.Attributes & (TypeAttributes.Interface | TypeAttributes.Abstract)) != 0) continue;
    if (!IsMessage(handle, new())) continue;
    bool sealedType = (type.Attributes & TypeAttributes.Sealed) != 0;
    var fields = type.GetFields().Select(fieldHandle => {
        var field = reader.GetFieldDefinition(fieldHandle);
        return new { token = MetadataTokens.GetToken(fieldHandle), name = reader.GetString(field.Name), flags = field.Attributes.ToString(), signature = Convert.ToHexString(reader.GetBlobBytes(field.Signature)) };
    }).Where(f => !f.flags.Contains("Static") && !f.flags.Contains("InitOnly") && !f.flags.Contains("Literal") && !f.flags.Contains("NotSerialized")).ToArray();
    rows.Add(new { token = MetadataTokens.GetToken(handle), name, sealed_type = sealedType, candidate_category = sealedType ? nextCategory++ : 0, fields });
}
Console.WriteLine(JsonSerializer.Serialize(new { source_sha256 = hash, method = "Read-only ECMA-335 metadata; no Assembly.Load or original code execution", category_status = "candidate: metadata type order + recovered Messages.Types and Extension.GetConverter; runtime/client order remains unverified", unresolved_type_specs = unresolved, messages = rows }, new JsonSerializerOptions { WriteIndented = true }));
