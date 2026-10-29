//! Guard dependency direction in the single crate without inventing extra packages.
use std::{fs, path::Path};
use syn::{
    ItemUse, Path as SynPath, UseTree,
    visit::{self, Visit},
};

#[derive(Default)]
struct Dependencies {
    domain: bool,
    violations: Vec<String>,
}

impl Dependencies {
    fn check(&mut self, segments: &[String]) {
        let Some(first) = segments.first().map(String::as_str) else {
            return;
        };
        let forbidden_external = ["axum", "sqlx", "serde", "serde_json", "tokio", "reqwest"];
        let bootstrap_or_adapter = first == "crate"
            && segments.get(1).is_some_and(|name| {
                ["adapters", "config", "auth", "database"].contains(&name.as_str())
            });
        let application_from_domain = self.domain
            && first == "crate"
            && segments.get(1).is_some_and(|name| name == "application");
        if forbidden_external.contains(&first) || bootstrap_or_adapter || application_from_domain {
            self.violations.push(segments.join("::"));
        }
    }

    fn use_tree(&mut self, tree: &UseTree, mut prefix: Vec<String>) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.use_tree(&path.tree, prefix);
            }
            UseTree::Name(name) => {
                prefix.push(name.ident.to_string());
                self.check(&prefix);
            }
            UseTree::Rename(rename) => {
                prefix.push(rename.ident.to_string());
                self.check(&prefix);
            }
            UseTree::Glob(_) => self.check(&prefix),
            UseTree::Group(group) => {
                for item in &group.items {
                    self.use_tree(item, prefix.clone());
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for Dependencies {
    fn visit_path(&mut self, path: &'ast SynPath) {
        self.check(
            &path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect::<Vec<_>>(),
        );
        visit::visit_path(self, path);
    }
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        self.use_tree(&item.tree, Vec::new());
        visit::visit_item_use(self, item);
    }
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if attribute.path().is_ident("derive") {
            let paths = attribute
                .parse_args_with(
                    syn::punctuated::Punctuated::<SynPath, syn::Token![,]>::parse_terminated,
                )
                .unwrap();
            for path in paths {
                self.visit_path(&path);
            }
        }
        visit::visit_attribute(self, attribute);
    }
}

fn check_directory(path: &Path, domain: bool) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            check_directory(&path, domain);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let file = syn::parse_file(&fs::read_to_string(&path).unwrap()).unwrap();
            let mut dependencies = Dependencies {
                domain,
                ..Dependencies::default()
            };
            dependencies.visit_file(&file);
            assert!(
                dependencies.violations.is_empty(),
                "Core dependency violation in {}: {:?}",
                path.display(),
                dependencies.violations
            );
        }
    }
}

#[test]
fn core_does_not_import_transport_persistence_or_bootstrap() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    check_directory(&root.join("domain"), true);
    check_directory(&root.join("application"), false);
}

#[test]
fn boundary_guard_detects_grouped_imports_derives_and_reversed_core_dependency() {
    for source in [
        "use crate::{adapters::outbound as database};",
        "#[derive(sqlx::FromRow)] struct Row;",
        "use serde as serialization;",
        "use crate::application::session_service::SessionService;",
    ] {
        let file = syn::parse_file(source).unwrap();
        let mut dependencies = Dependencies {
            domain: true,
            ..Dependencies::default()
        };
        dependencies.visit_file(&file);
        assert!(
            !dependencies.violations.is_empty(),
            "Boundary guard missed a dependency"
        );
    }
}
