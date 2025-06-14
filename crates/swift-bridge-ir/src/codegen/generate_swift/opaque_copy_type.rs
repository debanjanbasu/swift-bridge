use crate::codegen::generate_swift::generate_swift_class_methods;
use crate::parse::OpaqueForeignTypeDeclaration;
use crate::{ParsedExternFn, TypeDeclarations, SWIFT_BRIDGE_PREFIX};
use std::collections::HashMap;
use syn::Path;

pub(super) fn generate_opaque_copy_struct(
    ty: &OpaqueForeignTypeDeclaration,
    associated_funcs_and_methods: &HashMap<String, Vec<&ParsedExternFn>>,
    types: &TypeDeclarations,
    swift_bridge_path: &Path,
) -> String {
    let type_name = &ty.ty.to_string();

    let class_methods = generate_swift_class_methods(
        type_name,
        associated_funcs_and_methods,
        types,
        swift_bridge_path,
    );

    let mut extensions = "".to_string();

    append_methods_extension(&mut extensions, type_name, &class_methods.initializers);
    append_methods_extension(
        &mut extensions,
        type_name,
        &class_methods.owned_self_methods,
    );
    append_methods_extension(&mut extensions, type_name, &class_methods.ref_self_methods);

    if !class_methods.owned_self_methods.is_empty() {};

    let struct_definition = if !ty.attributes.already_declared {
        generate_struct_definition(ty, types, swift_bridge_path)
    } else {
        "".to_string()
    };

    format!(
        r#"{struct_definition}{extensions}"#
    )
}

fn generate_struct_definition(
    ty: &OpaqueForeignTypeDeclaration,
    types: &TypeDeclarations,
    swift_bridge_path: &Path,
) -> String {
    let type_name = ty.ty.to_string();
    let generics = ty.generics.angle_bracketed_generic_placeholders_string();

    let declare_struct = if ty.generics.is_empty() {
        format!(
            r#"public struct {type_name} {{
    fileprivate var bytes: {SWIFT_BRIDGE_PREFIX}${type_name}

    func intoFfiRepr() -> {SWIFT_BRIDGE_PREFIX}${type_name} {{
        bytes
    }}
}}"#,
        )
    } else {
        format!(
            r#"public struct {type_name}{generics} {{
    fileprivate var bytes: SwiftBridgeGenericCopyTypeFfiRepr
}}"#
        )
    };

    let ffi_repr_conversion = if ty.generics.is_empty() {
        format!(
            r#"extension {SWIFT_BRIDGE_PREFIX}${type_name} {{
    func intoSwiftRepr() -> {type_name} {{
        {type_name}(bytes: self)
    }}
}}"#,
        )
    } else {
        let ffi_repr_name = ty.ffi_repr_name_string();
        let bounds = ty
            .generics
            .rust_opaque_type_swift_generic_bounds(types, swift_bridge_path);

        format!(
            r#"extension {type_name}
where {bounds} {{
    func intoFfiRepr() -> {ffi_repr_name} {{
        self.bytes as! {ffi_repr_name}
    }}
}}
extension {ffi_repr_name} {{
    func intoSwiftRepr() -> {type_name}{generics} {{
        {type_name}(bytes: self)
    }}
}}
extension {ffi_repr_name}: SwiftBridgeGenericCopyTypeFfiRepr {{}}"#,
            ffi_repr_name = ffi_repr_name,
            type_name = type_name,
            bounds = bounds,
            generics = ty
                .generics
                .angle_bracketed_generic_concrete_swift_types_string(types, swift_bridge_path),
        )
    };

    let ext_equatable = if ty.attributes.equatable {
        format!(
            r#"
extension {type_name}: Equatable {{
    public static func == (lhs: Self, rhs: Self) -> Bool {{
        var lhs = lhs
        var rhs = rhs
        return withUnsafePointer(to: &lhs.bytes, {{(lhs_p: UnsafePointer<{ffi_repr_name}>) in
            return withUnsafePointer(to: &rhs.bytes, {{(rhs_p: UnsafePointer<{ffi_repr_name}>) in
                return __swift_bridge__${type_name}$_partial_eq(
                    UnsafeMutablePointer(mutating: lhs_p),
                    UnsafeMutablePointer(mutating: rhs_p)
                )
            }})
        }})
    }}
}}
"#,
            type_name = type_name,
            ffi_repr_name = ty.ffi_repr_name_string()
        )
    } else {
        String::new()
    };

    format!(
        r#"{declare_struct}
{ffi_repr_conversion}
{ext_equatable}"#,
    )
}

fn append_methods_extension(extensions: &mut String, type_name: &str, methods: &[String]) {
    if methods.is_empty() {
        return;
    }

    *extensions += &format!(
        r#"
extension {type_name} {{
"#
    );

    for (idx, method) in methods.iter().enumerate() {
        if idx > 0 {
            *extensions += "\n";
        }

        *extensions += method;
    }

    *extensions += "\n}";
}
