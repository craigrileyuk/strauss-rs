//! How the names in a file will resolve once it has been rewritten.
//!
//! A name in code is read relative to the file's namespace and `use`
//! statements. When those change, a name that wasn't touched can start to mean
//! something else. This rebuilds the file's namespace and imports as they will
//! be, so a name can be checked before deciding whether to change it.

use php_ast::ast::UseKind;
use php_ast::resolve::{NameContext, NameResolver, ResolvedName};

use crate::analyser::{ImportKind, resolve_written_name};
use crate::edits::names::{Group, NameMap};

/// An import as it will be written.
#[derive(Debug, Clone)]
pub(super) struct NewImport {
    pub kind: ImportKind,

    /// The name it imports once everything is renamed.
    pub name: String,

    /// What the file calls it: the alias it has, or one added so that the
    /// code that uses it keeps working when the end of its name changes.
    pub alias: Option<String>,

    /// An alias that has to be written because there wasn't one.
    pub added_alias: Option<String>,
}

pub(super) struct NewContext<'a> {
    resolver: NameResolver,
    names: &'a NameMap,
}

impl<'a> NewContext<'a> {
    pub fn new(names: &'a NameMap, namespace: Option<&str>, imports: &[NewImport]) -> Self {
        let mut resolver = NameResolver::new();

        resolver.enter_namespace(namespace);

        for import in imports {
            let kind = match import.kind {
                ImportKind::Class => UseKind::Normal,
                ImportKind::Function => UseKind::Function,
                ImportKind::Constant => UseKind::Const,
            };

            resolver.add_import(kind, &import.name, import.alias.as_deref());
        }

        Self { resolver, names }
    }

    /// Whether `written`, in the rewritten file, means `target`.
    ///
    /// An unqualified function or constant in a namespace means the one in the
    /// namespace if there is one, else the global one.
    pub fn means(&self, group: Group, written: &str, target: &str) -> bool {
        let context = match group {
            Group::ClassLike => NameContext::Class,
            Group::Function => NameContext::Function,
            Group::Constant => NameContext::Const,
        };

        match resolve_written_name(&self.resolver, context, written) {
            ResolvedName::Fqn(name) => group.same(&name, target),

            ResolvedName::Fallback { namespaced, global } => {
                group.same(&namespaced, target)
                    || (group.same(&global, target) && !self.names.exists_after(group, &namespaced))
            }

            ResolvedName::Special(_) | ResolvedName::Error => false,
        }
    }
}
