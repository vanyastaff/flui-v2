use std::any::{Any, TypeId, type_name};

use collections::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::{EntityId, GlobalElementId};

use super::InheritedValue;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ProviderScopeKey {
    type_id: TypeId,
    scope_id: GlobalElementId,
}

impl ProviderScopeKey {
    pub(crate) fn of<T: InheritedValue>(scope_id: GlobalElementId) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            scope_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InheritedDependency {
    pub(crate) provider: ProviderScopeKey,
    pub(crate) provider_version: u64,
    pub(crate) dependent_element: GlobalElementId,
    pub(crate) dependent_view: EntityId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InheritedDependent {
    pub(crate) element_id: GlobalElementId,
    pub(crate) view_id: EntityId,
    pub(crate) last_seen_version: u64,
}

struct InheritedEntry {
    value: Box<dyn Any + Send + Sync>,
    version: u64,
    dependents: SmallVec<[InheritedDependent; 4]>,
}

impl InheritedEntry {
    fn new<T: InheritedValue>(value: T) -> Self {
        Self {
            value: Box::new(value),
            version: 0,
            dependents: SmallVec::new(),
        }
    }

    fn value<T: InheritedValue>(&self) -> &T {
        self.value.downcast_ref::<T>().unwrap_or_else(|| {
            panic!(
                "provider registry stored the wrong value type for {}",
                type_name::<T>()
            )
        })
    }

    fn replace<T: InheritedValue>(&mut self, value: T) {
        self.value = Box::new(value);
        self.version = self.version.saturating_add(1);
    }

    fn upsert_dependent(&mut self, dependent: InheritedDependent) {
        if let Some(existing) = self.dependents.iter_mut().find(|existing| {
            existing.element_id == dependent.element_id && existing.view_id == dependent.view_id
        }) {
            *existing = dependent;
        } else {
            self.dependents.push(dependent);
        }
    }

    fn dependent_views(&self) -> SmallVec<[EntityId; 4]> {
        let mut views = SmallVec::new();

        for dependent in &self.dependents {
            push_unique_view(&mut views, dependent.view_id);
        }

        views
    }
}

#[derive(Default)]
pub(crate) struct InheritedRegistry {
    entries: FxHashMap<ProviderScopeKey, InheritedEntry>,
    active_by_type: FxHashMap<TypeId, SmallVec<[GlobalElementId; 4]>>,
    accessed_providers: FxHashSet<ProviderScopeKey>,
    accessed_provider_order: Vec<ProviderScopeKey>,
    accessed_dependencies: Vec<InheritedDependency>,
}

impl InheritedRegistry {
    pub(crate) fn begin_frame(&mut self) {
        self.active_by_type.clear();
        self.accessed_providers.clear();
        self.accessed_provider_order.clear();
        self.accessed_dependencies.clear();
    }

    pub(crate) fn provide<T: InheritedValue>(
        &mut self,
        scope_id: &GlobalElementId,
        value: &T,
    ) -> SmallVec<[EntityId; 4]> {
        let key = ProviderScopeKey::of::<T>(scope_id.clone());
        self.mark_provider_accessed(key.clone());

        let Some(entry) = self.entries.get_mut(&key) else {
            self.entries
                .insert(key, InheritedEntry::new::<T>(value.clone()));
            return SmallVec::new();
        };

        if entry.value::<T>() == value {
            return SmallVec::new();
        }

        let dirty_views = entry.dependent_views();
        entry.replace::<T>(value.clone());
        dirty_views
    }

    pub(crate) fn push_active<T: InheritedValue>(&mut self, scope_id: &GlobalElementId) {
        let key = ProviderScopeKey::of::<T>(scope_id.clone());
        debug_assert!(
            self.entries.contains_key(&key),
            "Provider<{}> must be registered before activation",
            type_name::<T>()
        );
        self.mark_provider_accessed(key);
        self.active_by_type
            .entry(TypeId::of::<T>())
            .or_default()
            .push(scope_id.clone());
    }

    pub(crate) fn pop_active<T: InheritedValue>(&mut self, scope_id: &GlobalElementId) {
        let type_id = TypeId::of::<T>();
        let is_empty = {
            let stack = self.active_by_type.get_mut(&type_id).unwrap_or_else(|| {
                panic!(
                    "Provider<{}> popped without an active provider stack",
                    type_name::<T>()
                )
            });
            let popped = stack.pop().unwrap_or_else(|| {
                panic!(
                    "Provider<{}> popped from an empty provider stack",
                    type_name::<T>()
                )
            });
            assert_eq!(
                popped,
                *scope_id,
                "Provider<{}> activation stack was restored out of order",
                type_name::<T>()
            );
            stack.is_empty()
        };

        if is_empty {
            self.active_by_type.remove(&type_id);
        }
    }

    pub(crate) fn read<T: InheritedValue>(&self) -> Option<T> {
        let scope_id = self.active_scope_id::<T>()?;
        let key = ProviderScopeKey::of::<T>(scope_id.clone());
        let entry = self.entries.get(&key)?;
        Some(entry.value::<T>().clone())
    }

    pub(crate) fn inherit<T: InheritedValue>(
        &mut self,
        dependent_element: &GlobalElementId,
        dependent_view: EntityId,
    ) -> Option<T> {
        let scope_id = self.active_scope_id::<T>()?.clone();
        let key = ProviderScopeKey::of::<T>(scope_id);
        self.record_dependency::<T>(key, dependent_element, dependent_view)
    }

    pub(crate) fn replay_dependency(
        &mut self,
        dependency: InheritedDependency,
    ) -> SmallVec<[EntityId; 4]> {
        let mut dirty_views = SmallVec::new();

        if !self.accessed_providers.contains(&dependency.provider) {
            push_unique_view(&mut dirty_views, dependency.dependent_view);
            return dirty_views;
        }

        let Some(entry) = self.entries.get_mut(&dependency.provider) else {
            push_unique_view(&mut dirty_views, dependency.dependent_view);
            return dirty_views;
        };

        if entry.version != dependency.provider_version {
            push_unique_view(&mut dirty_views, dependency.dependent_view);
        }

        entry.upsert_dependent(InheritedDependent {
            element_id: dependency.dependent_element.clone(),
            view_id: dependency.dependent_view,
            last_seen_version: entry.version,
        });

        self.accessed_dependencies.push(dependency);
        dirty_views
    }

    pub(crate) fn replay_provider_access(&mut self, provider: ProviderScopeKey) {
        if self.entries.contains_key(&provider) {
            self.mark_provider_accessed(provider);
        }
    }

    pub(crate) fn remove_unaccessed_providers(&mut self) -> SmallVec<[EntityId; 8]> {
        let mut dirty_views = SmallVec::new();

        let accessed_providers = &self.accessed_providers;
        self.entries.retain(|key, entry| {
            if accessed_providers.contains(key) {
                return true;
            }

            for view_id in entry.dependent_views() {
                push_unique_view(&mut dirty_views, view_id);
            }
            false
        });

        self.prune_unaccessed_dependents();

        dirty_views
    }

    pub(crate) fn accessed_provider_index(&self) -> usize {
        self.accessed_provider_order.len()
    }

    pub(crate) fn accessed_providers_since(&self, index: usize) -> Vec<ProviderScopeKey> {
        self.accessed_provider_order[index..].to_vec()
    }

    pub(crate) fn accessed_dependency_index(&self) -> usize {
        self.accessed_dependencies.len()
    }

    pub(crate) fn accessed_dependencies_since(&self, index: usize) -> Vec<InheritedDependency> {
        self.accessed_dependencies[index..].to_vec()
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) fn accessed_dependencies(&self) -> &[InheritedDependency] {
        &self.accessed_dependencies
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) fn provider_version(&self, key: &ProviderScopeKey) -> Option<u64> {
        self.entries.get(key).map(|entry| entry.version)
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) fn dependent_count(&self, key: &ProviderScopeKey) -> usize {
        self.entries
            .get(key)
            .map_or(0, |entry| entry.dependents.len())
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) fn contains_provider(&self, key: &ProviderScopeKey) -> bool {
        self.entries.contains_key(key)
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) fn active_scope_count<T: InheritedValue>(&self) -> usize {
        self.active_by_type
            .get(&TypeId::of::<T>())
            .map_or(0, SmallVec::len)
    }

    #[cfg(any(test, feature = "test-support"))]
    #[allow(dead_code)]
    pub(crate) fn active_scope<T: InheritedValue>(&self) -> Option<&GlobalElementId> {
        self.active_scope_id::<T>()
    }

    fn active_scope_id<T: InheritedValue>(&self) -> Option<&GlobalElementId> {
        self.active_by_type.get(&TypeId::of::<T>())?.last()
    }

    fn record_dependency<T: InheritedValue>(
        &mut self,
        key: ProviderScopeKey,
        dependent_element: &GlobalElementId,
        dependent_view: EntityId,
    ) -> Option<T> {
        let entry = self.entries.get_mut(&key)?;
        let value = entry.value::<T>().clone();
        let provider_version = entry.version;

        entry.upsert_dependent(InheritedDependent {
            element_id: dependent_element.clone(),
            view_id: dependent_view,
            last_seen_version: provider_version,
        });

        self.accessed_dependencies.push(InheritedDependency {
            provider: key,
            provider_version,
            dependent_element: dependent_element.clone(),
            dependent_view,
        });

        Some(value)
    }

    fn mark_provider_accessed(&mut self, key: ProviderScopeKey) {
        if self.accessed_providers.insert(key.clone()) {
            self.accessed_provider_order.push(key);
        }
    }

    fn prune_unaccessed_dependents(&mut self) {
        let accessed_dependencies = &self.accessed_dependencies;

        for (key, entry) in &mut self.entries {
            entry.dependents.retain(|dependent| {
                accessed_dependencies.iter().any(|accessed| {
                    accessed.provider == *key
                        && accessed.dependent_element == dependent.element_id
                        && accessed.dependent_view == dependent.view_id
                })
            });
        }
    }
}

fn push_unique_view<const N: usize>(views: &mut SmallVec<[EntityId; N]>, view_id: EntityId) {
    if !views.contains(&view_id) {
        views.push(view_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::ElementId;

    fn scope(name: &str) -> GlobalElementId {
        GlobalElementId(Arc::from(
            vec![ElementId::Name(name.to_string().into())].into_boxed_slice(),
        ))
    }

    fn view(id: u64) -> EntityId {
        EntityId::from(id)
    }

    #[test]
    fn reads_nearest_active_provider() {
        let mut registry = InheritedRegistry::default();
        let outer = scope("outer");
        let inner = scope("inner");

        registry.provide::<i32>(&outer, &1);
        registry.push_active::<i32>(&outer);
        assert_eq!(registry.active_scope_count::<i32>(), 1);
        assert_eq!(registry.active_scope::<i32>(), Some(&outer));
        assert_eq!(registry.read::<i32>(), Some(1));

        registry.provide::<i32>(&inner, &2);
        registry.push_active::<i32>(&inner);
        assert_eq!(registry.active_scope_count::<i32>(), 2);
        assert_eq!(registry.active_scope::<i32>(), Some(&inner));
        assert_eq!(registry.read::<i32>(), Some(2));

        registry.pop_active::<i32>(&inner);
        assert_eq!(registry.active_scope_count::<i32>(), 1);
        assert_eq!(registry.active_scope::<i32>(), Some(&outer));
        assert_eq!(registry.read::<i32>(), Some(1));

        registry.pop_active::<i32>(&outer);
        assert_eq!(registry.active_scope_count::<i32>(), 0);
        assert_eq!(registry.read::<i32>(), None);
    }

    #[test]
    fn tracks_multiple_value_types() {
        let mut registry = InheritedRegistry::default();
        let number = scope("number");
        let text = scope("text");

        registry.provide::<i32>(&number, &42);
        registry.provide::<String>(&text, &"hello".to_string());
        registry.push_active::<i32>(&number);
        registry.push_active::<String>(&text);

        assert_eq!(registry.read::<i32>(), Some(42));
        assert_eq!(registry.read::<String>(), Some("hello".to_string()));
    }

    #[test]
    fn separate_registries_do_not_share_active_providers() {
        let mut first = InheritedRegistry::default();
        let second = InheritedRegistry::default();
        let provider = scope("provider");

        first.provide::<i32>(&provider, &42);
        first.push_active::<i32>(&provider);

        assert_eq!(first.read::<i32>(), Some(42));
        assert_eq!(second.read::<i32>(), None);
    }

    #[test]
    fn inherit_records_and_deduplicates_dependents() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let dependent = scope("dependent");
        let key = ProviderScopeKey::of::<i32>(provider.clone());

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);

        assert_eq!(registry.inherit::<i32>(&dependent, view(1)), Some(7));
        assert_eq!(registry.inherit::<i32>(&dependent, view(1)), Some(7));

        assert_eq!(registry.dependent_count(&key), 1);
        assert_eq!(registry.accessed_dependencies().len(), 2);
    }

    #[test]
    fn unchanged_values_do_not_dirty_dependents_or_bump_version() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let dependent = scope("dependent");
        let key = ProviderScopeKey::of::<i32>(provider.clone());

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);
        registry.inherit::<i32>(&dependent, view(1));

        let dirty_views = registry.provide::<i32>(&provider, &7);

        assert!(dirty_views.is_empty());
        assert_eq!(registry.provider_version(&key), Some(0));
    }

    #[test]
    fn changed_values_dirty_dependents_and_bump_version() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let dependent = scope("dependent");
        let key = ProviderScopeKey::of::<i32>(provider.clone());

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);
        registry.inherit::<i32>(&dependent, view(1));

        let dirty_views = registry.provide::<i32>(&provider, &8);

        assert_eq!(dirty_views.as_slice(), &[view(1)]);
        assert_eq!(registry.provider_version(&key), Some(1));
    }

    #[test]
    fn replay_dependency_keeps_cached_dependent_live() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let dependent = scope("dependent");
        let key = ProviderScopeKey::of::<i32>(provider.clone());

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);
        registry.inherit::<i32>(&dependent, view(1));
        let dependency = registry.accessed_dependencies()[0].clone();

        registry.begin_frame();
        registry.replay_provider_access(key.clone());
        let dirty_views = registry.replay_dependency(dependency);

        assert!(dirty_views.is_empty());
        assert_eq!(registry.dependent_count(&key), 1);
        assert!(registry.remove_unaccessed_providers().is_empty());
        assert!(registry.contains_provider(&key));
    }

    #[test]
    fn replaying_dependency_without_live_provider_dirties_dependent() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let dependent = scope("dependent");
        let key = ProviderScopeKey::of::<i32>(provider.clone());

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);
        registry.inherit::<i32>(&dependent, view(1));
        let dependency = registry.accessed_dependencies()[0].clone();

        registry.begin_frame();
        let dirty_views = registry.replay_dependency(dependency);

        assert_eq!(dirty_views.as_slice(), &[view(1)]);
        assert_eq!(
            registry.remove_unaccessed_providers().as_slice(),
            &[view(1)]
        );
        assert!(!registry.contains_provider(&key));
    }

    #[test]
    fn replaying_stale_cached_dependency_marks_view_dirty() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let dependent = scope("dependent");

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);
        registry.inherit::<i32>(&dependent, view(1));
        let dependency = registry.accessed_dependencies()[0].clone();

        registry.provide::<i32>(&provider, &8);
        registry.begin_frame();
        registry.replay_provider_access(ProviderScopeKey::of::<i32>(provider));
        let dirty_views = registry.replay_dependency(dependency);

        assert_eq!(dirty_views.as_slice(), &[view(1)]);
    }

    #[test]
    fn removing_unaccessed_provider_invalidates_previous_dependents_once() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let dependent = scope("dependent");
        let key = ProviderScopeKey::of::<i32>(provider.clone());

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);
        registry.inherit::<i32>(&dependent, view(1));
        registry.pop_active::<i32>(&provider);

        registry.begin_frame();
        assert_eq!(
            registry.remove_unaccessed_providers().as_slice(),
            &[view(1)]
        );
        assert!(!registry.contains_provider(&key));
        assert!(registry.remove_unaccessed_providers().is_empty());
    }

    #[test]
    fn cleanup_prunes_dependents_that_are_not_replayed() {
        let mut registry = InheritedRegistry::default();
        let provider = scope("provider");
        let first_dependent = scope("first-dependent");
        let second_dependent = scope("second-dependent");
        let key = ProviderScopeKey::of::<i32>(provider.clone());

        registry.provide::<i32>(&provider, &7);
        registry.push_active::<i32>(&provider);
        registry.inherit::<i32>(&first_dependent, view(1));
        registry.inherit::<i32>(&second_dependent, view(2));
        let dependency = registry.accessed_dependencies()[0].clone();

        registry.begin_frame();
        registry.replay_provider_access(key.clone());
        registry.replay_dependency(dependency);
        assert!(registry.remove_unaccessed_providers().is_empty());

        assert_eq!(registry.dependent_count(&key), 1);
    }

    #[test]
    #[should_panic(expected = "activation stack was restored out of order")]
    fn pop_active_panics_when_scope_order_is_wrong() {
        let mut registry = InheritedRegistry::default();
        let outer = scope("outer");
        let inner = scope("inner");

        registry.provide::<i32>(&outer, &1);
        registry.provide::<i32>(&inner, &2);
        registry.push_active::<i32>(&outer);
        registry.push_active::<i32>(&inner);

        registry.pop_active::<i32>(&outer);
    }
}
