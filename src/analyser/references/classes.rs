//! References made by class-like declarations: classes, interfaces, traits and enums.

use crate::analyser::context::AnalysisContext;
use crate::analyser::model::{Reference, ReferenceKind};
use crate::analyser::references::expressions::collect_expr;
use crate::analyser::references::names::build_reference;
use crate::analyser::references::signatures::{
    collect_attributes, collect_method, collect_property_hooks, collect_type_hint,
};
use php_ast::{
    owned::{
        ClassConstDecl, ClassDecl, ClassMember, ClassMemberKind, EnumDecl, EnumMemberKind,
        InterfaceDecl, Name, PropertyDecl, TraitDecl, TraitUseDecl,
    },
    resolve::NameContext,
};

pub(super) fn collect_class(
    class: &ClassDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&class.attributes, context, references);

    if let Some(parent) = &class.extends {
        collect_class_name(parent, context, references);
    }

    for interface in class.implements.iter() {
        collect_class_name(interface, context, references);
    }

    for member in class.body.members.iter() {
        collect_class_member(member, context, references);
    }
}

pub(super) fn collect_class_name(
    name: &Name,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    let resolved = context
        .resolver
        .resolve_owned_name(name, NameContext::Class);

    references.extend(build_reference(
        resolved,
        name.span,
        ReferenceKind::ClassName,
    ));
}

fn collect_class_member(
    member: &ClassMember,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    match &member.kind {
        ClassMemberKind::Property(property) => {
            collect_property(property, context, references);
        }

        ClassMemberKind::Method(method) => {
            collect_method(method, context, references);
        }

        ClassMemberKind::TraitUse(trait_use) => {
            collect_trait_use(trait_use, context, references);
        }

        ClassMemberKind::ClassConst(class_const) => {
            collect_class_const(class_const, context, references);
        }
    }
}

fn collect_class_const(
    class_const: &ClassConstDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&class_const.attributes, context, references);

    if let Some(type_hint) = &class_const.type_hint {
        collect_type_hint(type_hint, context, references);
    }

    collect_expr(&class_const.value, context, references);
}

fn collect_property(
    property: &PropertyDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&property.attributes, context, references);

    if let Some(type_hint) = &property.type_hint {
        collect_type_hint(type_hint, context, references);
    }

    if let Some(default) = &property.default {
        collect_expr(default, context, references);
    }

    collect_property_hooks(&property.hooks, context, references);
}

pub(super) fn collect_interface(
    interface: &InterfaceDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&interface.attributes, context, references);

    for parent in interface.extends.iter() {
        collect_class_name(parent, context, references);
    }

    for member in interface.body.members.iter() {
        collect_class_member(member, context, references);
    }
}

pub(super) fn collect_trait(
    trait_decl: &TraitDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&trait_decl.attributes, context, references);

    for member in trait_decl.body.members.iter() {
        collect_class_member(member, context, references);
    }
}

fn collect_trait_use(
    trait_use: &TraitUseDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    for name in trait_use.traits.iter() {
        collect_class_name(name, context, references);
    }
}

pub(super) fn collect_enum(
    enum_decl: &EnumDecl,
    context: &AnalysisContext,
    references: &mut Vec<Reference>,
) {
    collect_attributes(&enum_decl.attributes, context, references);

    for interface in enum_decl.implements.iter() {
        collect_class_name(interface, context, references);
    }

    for member in enum_decl.body.members.iter() {
        match &member.kind {
            EnumMemberKind::Method(method) => {
                collect_method(method, context, references);
            }

            EnumMemberKind::TraitUse(trait_use) => {
                collect_trait_use(trait_use, context, references);
            }

            EnumMemberKind::ClassConst(class_const) => {
                collect_class_const(class_const, context, references);
            }

            EnumMemberKind::Case(case) => {
                collect_attributes(&case.attributes, context, references);

                if let Some(value) = &case.value {
                    collect_expr(value, context, references);
                }
            }
        }
    }
}
