use crate::{EntityId, FocusHandle, FocusId, SharedString};
use smallvec::SmallVec;
use std::{
    fmt::{self, Display},
    num::TryFromIntError,
    ops::Deref,
    sync::Arc,
};
use uuid::Uuid;

/// User-facing identity intent for an element.
///
/// `Key` is the compatibility bridge for the future Framework tier. The engine still stores
/// normalized [`ElementId`] path segments in [`crate::GlobalElementId`].
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Key(KeyKind);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum KeyKind {
    Local(core::panic::Location<'static>),
    Value(ElementId),
    Global(GlobalKey),
}

impl Key {
    /// Creates a Local key from the caller location.
    ///
    /// Local identity is scoped by the parent element and disambiguated by sibling occurrence. It
    /// is not reorder-stable; use [`Key::value`] for list items or other reordered children.
    #[track_caller]
    pub fn local() -> Self {
        Self(KeyKind::Local(*core::panic::Location::caller()))
    }

    /// Creates a Value key from a reorder-stable value.
    pub fn value(value: impl Into<ValueKey>) -> Self {
        Self(KeyKind::Value(value.into().into_element_id()))
    }

    /// Creates a Global key.
    pub fn global(value: impl Into<GlobalKey>) -> Self {
        Self(KeyKind::Global(value.into()))
    }
}

impl From<Key> for ElementId {
    fn from(key: Key) -> Self {
        match key.0 {
            KeyKind::Local(location) => ElementId::CodeLocation(location),
            KeyKind::Value(value) => value,
            KeyKind::Global(global_key) => ElementId::GlobalKey(global_key),
        }
    }
}

impl From<GlobalKey> for Key {
    fn from(key: GlobalKey) -> Self {
        Key(KeyKind::Global(key))
    }
}

/// Reorder-stable value identity accepted by [`Key::value`].
///
/// This intentionally supports the existing value-like `ElementId` conversion set without accepting
/// Local or Global key inputs.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ValueKey(ElementId);

impl ValueKey {
    /// Returns the engine path segment backing this value key.
    pub fn into_element_id(self) -> ElementId {
        self.0
    }
}

impl From<usize> for ValueKey {
    fn from(id: usize) -> Self {
        Self(ElementId::from(id))
    }
}

impl TryFrom<i32> for ValueKey {
    type Error = TryFromIntError;

    fn try_from(id: i32) -> Result<Self, Self::Error> {
        ElementId::try_from(id).map(Self)
    }
}

impl From<SharedString> for ValueKey {
    fn from(name: SharedString) -> Self {
        Self(ElementId::from(name))
    }
}

impl From<String> for ValueKey {
    fn from(name: String) -> Self {
        Self(ElementId::from(name))
    }
}

impl From<Arc<str>> for ValueKey {
    fn from(name: Arc<str>) -> Self {
        Self(ElementId::from(name))
    }
}

impl From<Arc<std::path::Path>> for ValueKey {
    fn from(path: Arc<std::path::Path>) -> Self {
        Self(ElementId::from(path))
    }
}

impl From<&'static str> for ValueKey {
    fn from(name: &'static str) -> Self {
        Self(ElementId::from(name))
    }
}

impl<'a> From<&'a FocusHandle> for ValueKey {
    fn from(handle: &'a FocusHandle) -> Self {
        Self(ElementId::from(handle))
    }
}

impl From<(&'static str, EntityId)> for ValueKey {
    fn from(value: (&'static str, EntityId)) -> Self {
        Self(ElementId::from(value))
    }
}

impl From<(&'static str, usize)> for ValueKey {
    fn from(value: (&'static str, usize)) -> Self {
        Self(ElementId::from(value))
    }
}

impl From<(SharedString, usize)> for ValueKey {
    fn from(value: (SharedString, usize)) -> Self {
        Self(ElementId::from(value))
    }
}

impl From<(&'static str, u64)> for ValueKey {
    fn from(value: (&'static str, u64)) -> Self {
        Self(ElementId::from(value))
    }
}

impl From<Uuid> for ValueKey {
    fn from(value: Uuid) -> Self {
        Self(ElementId::from(value))
    }
}

impl From<(&'static str, u32)> for ValueKey {
    fn from(value: (&'static str, u32)) -> Self {
        Self(ElementId::from(value))
    }
}

impl<T: Into<SharedString>> From<(ValueKey, T)> for ValueKey {
    fn from((id, name): (ValueKey, T)) -> Self {
        Self(ElementId::NamedChild(
            Arc::new(id.into_element_id()),
            name.into(),
        ))
    }
}

/// A caller source location plus sibling occurrence in the parent namespace.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LocalElementId {
    source_location: core::panic::Location<'static>,
    occurrence: u32,
}

impl LocalElementId {
    fn new(source_location: core::panic::Location<'static>, occurrence: u32) -> Self {
        Self {
            source_location,
            occurrence,
        }
    }

    /// The callsite that produced this local identity.
    pub fn source_location(&self) -> core::panic::Location<'static> {
        self.source_location
    }

    /// The occurrence of this callsite within the current parent namespace.
    pub fn occurrence(&self) -> u32 {
        self.occurrence
    }
}

/// A globally-scoped key value.
///
/// K02 only stores and compares global keys. Cross-tree move semantics are Framework-tier work.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct GlobalKey(SharedString);

impl GlobalKey {
    /// Creates a global key from a stable string-like value.
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self(value.into())
    }

    /// Returns the underlying key text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for GlobalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<SharedString> for GlobalKey {
    fn from(value: SharedString) -> Self {
        Self(value)
    }
}

impl From<String> for GlobalKey {
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<Arc<str>> for GlobalKey {
    fn from(value: Arc<str>) -> Self {
        Self(value.into())
    }
}

impl From<&'static str> for GlobalKey {
    fn from(value: &'static str) -> Self {
        Self(value.into())
    }
}

impl From<Uuid> for GlobalKey {
    fn from(value: Uuid) -> Self {
        Self(value.to_string().into())
    }
}

/// An identifier for an [`crate::Element`].
///
/// `ElementId` is a normalized engine path segment. Existing value-style constructors remain
/// supported. `CodeLocation` is a compatibility input and is rewritten to `Local` by
/// [`ElementIdStack`].
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ElementId {
    /// The ID of a View element.
    View(EntityId),
    /// An integer ID.
    Integer(u64),
    /// A string based ID.
    Name(SharedString),
    /// A UUID.
    Uuid(Uuid),
    /// An ID that's equated with a focus handle.
    FocusHandle(FocusId),
    /// A combination of a name and an integer.
    NamedInteger(SharedString, u64),
    /// A path.
    Path(Arc<std::path::Path>),
    /// A caller source location before stack normalization.
    CodeLocation(core::panic::Location<'static>),
    /// A caller source location plus sibling occurrence in the parent namespace.
    Local(LocalElementId),
    /// A globally-scoped key.
    GlobalKey(GlobalKey),
    /// A labeled child of an element.
    NamedChild(Arc<ElementId>, SharedString),
}

impl ElementId {
    /// Constructs an `ElementId::NamedInteger` from a name and `usize`.
    pub fn named_usize(name: impl Into<SharedString>, integer: usize) -> ElementId {
        Self::NamedInteger(name.into(), integer as u64)
    }
}

impl Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementId::View(entity_id) => write!(f, "view-{}", entity_id)?,
            ElementId::Integer(ix) => write!(f, "{}", ix)?,
            ElementId::Name(name) => write!(f, "{}", name)?,
            ElementId::FocusHandle(_) => write!(f, "FocusHandle")?,
            ElementId::NamedInteger(s, i) => write!(f, "{}-{}", s, i)?,
            ElementId::Uuid(uuid) => write!(f, "{}", uuid)?,
            ElementId::Path(path) => write!(f, "{}", path.display())?,
            ElementId::CodeLocation(location) => write!(f, "{}", location)?,
            ElementId::Local(local) => {
                write!(f, "{}#{}", local.source_location(), local.occurrence())?
            }
            ElementId::GlobalKey(global_key) => write!(f, "global:{}", global_key)?,
            ElementId::NamedChild(id, name) => write!(f, "{}-{}", id, name)?,
        }

        Ok(())
    }
}

impl TryInto<SharedString> for ElementId {
    type Error = anyhow::Error;

    fn try_into(self) -> anyhow::Result<SharedString> {
        if let ElementId::Name(name) = self {
            Ok(name)
        } else {
            anyhow::bail!("element id is not string")
        }
    }
}

impl From<GlobalKey> for ElementId {
    fn from(value: GlobalKey) -> Self {
        ElementId::GlobalKey(value)
    }
}

impl From<usize> for ElementId {
    fn from(id: usize) -> Self {
        ElementId::Integer(id as u64)
    }
}

impl TryFrom<i32> for ElementId {
    type Error = TryFromIntError;

    fn try_from(id: i32) -> Result<Self, Self::Error> {
        Ok(Self::Integer(u64::try_from(id)?))
    }
}

impl From<SharedString> for ElementId {
    fn from(name: SharedString) -> Self {
        ElementId::Name(name)
    }
}

impl From<String> for ElementId {
    fn from(name: String) -> Self {
        ElementId::Name(name.into())
    }
}

impl From<Arc<str>> for ElementId {
    fn from(name: Arc<str>) -> Self {
        ElementId::Name(name.into())
    }
}

impl From<Arc<std::path::Path>> for ElementId {
    fn from(path: Arc<std::path::Path>) -> Self {
        ElementId::Path(path)
    }
}

impl From<&'static str> for ElementId {
    fn from(name: &'static str) -> Self {
        ElementId::Name(name.into())
    }
}

impl<'a> From<&'a FocusHandle> for ElementId {
    fn from(handle: &'a FocusHandle) -> Self {
        ElementId::FocusHandle(handle.id)
    }
}

impl From<(&'static str, EntityId)> for ElementId {
    fn from((name, id): (&'static str, EntityId)) -> Self {
        ElementId::NamedInteger(name.into(), id.as_u64())
    }
}

impl From<(&'static str, usize)> for ElementId {
    fn from((name, id): (&'static str, usize)) -> Self {
        ElementId::NamedInteger(name.into(), id as u64)
    }
}

impl From<(SharedString, usize)> for ElementId {
    fn from((name, id): (SharedString, usize)) -> Self {
        ElementId::NamedInteger(name, id as u64)
    }
}

impl From<(&'static str, u64)> for ElementId {
    fn from((name, id): (&'static str, u64)) -> Self {
        ElementId::NamedInteger(name.into(), id)
    }
}

impl From<Uuid> for ElementId {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}

impl From<(&'static str, u32)> for ElementId {
    fn from((name, id): (&'static str, u32)) -> Self {
        ElementId::NamedInteger(name.into(), id.into())
    }
}

impl<T: Into<SharedString>> From<(ElementId, T)> for ElementId {
    fn from((id, name): (ElementId, T)) -> Self {
        ElementId::NamedChild(Arc::new(id), name.into())
    }
}

impl From<&'static core::panic::Location<'static>> for ElementId {
    fn from(location: &'static core::panic::Location<'static>) -> Self {
        ElementId::CodeLocation(*location)
    }
}

/// Stack of normalized element identity path segments.
///
/// The stack stores both the current path and the parent-scoped resolver state needed to turn
/// caller locations into deterministic Local segments.
type LocalOccurrences = SmallVec<[(core::panic::Location<'static>, u32); 4]>;
#[cfg(debug_assertions)]
type ExplicitSiblings = SmallVec<[ElementId; 8]>;

#[derive(Clone, Debug)]
pub(crate) struct ElementIdStack {
    path: SmallVec<[ElementId; 32]>,
    local_occurrences: SmallVec<[LocalOccurrences; 32]>,
    #[cfg(debug_assertions)]
    explicit_siblings: SmallVec<[ExplicitSiblings; 32]>,
}

impl Default for ElementIdStack {
    fn default() -> Self {
        let mut this = Self {
            path: SmallVec::new(),
            local_occurrences: SmallVec::new(),
            #[cfg(debug_assertions)]
            explicit_siblings: SmallVec::new(),
        };
        this.push_child_scope();
        this
    }
}

impl ElementIdStack {
    /// Starts a new lifecycle pass without changing the current path.
    pub(crate) fn begin_pass(&mut self) {
        assert!(
            self.path.is_empty(),
            "identity pass reset requires an empty element path"
        );
        let scope_count = self.path.len() + 1;
        self.local_occurrences.clear();
        self.local_occurrences
            .resize_with(scope_count, LocalOccurrences::default);
        #[cfg(debug_assertions)]
        {
            self.explicit_siblings.clear();
            self.explicit_siblings
                .resize_with(scope_count, ExplicitSiblings::default);
        }
    }

    /// Pushes and normalizes an element id.
    pub(crate) fn push(&mut self, element_id: impl Into<ElementId>) {
        let element_id = self.normalize(element_id.into());
        self.push_resolved(element_id);
    }

    /// Pushes an element id that was normalized during an earlier lifecycle pass.
    pub(crate) fn push_resolved(&mut self, element_id: ElementId) {
        debug_assert!(
            !matches!(element_id, ElementId::CodeLocation(_)),
            "resolved element ids must not contain raw CodeLocation segments"
        );
        self.record_explicit_sibling(&element_id);
        self.path.push(element_id);
        self.push_child_scope();
    }

    /// Pops the current element id.
    pub(crate) fn pop(&mut self) -> Option<ElementId> {
        debug_assert_eq!(self.local_occurrences.len(), self.path.len() + 1);
        let element_id = self.path.pop()?;
        self.local_occurrences.pop();
        #[cfg(debug_assertions)]
        self.explicit_siblings.pop();
        debug_assert_eq!(self.local_occurrences.len(), self.path.len() + 1);
        Some(element_id)
    }

    /// Clears the path and resolver state.
    pub(crate) fn clear(&mut self) {
        self.path.clear();
        self.begin_pass();
    }

    /// Returns the number of path segments.
    pub(crate) fn len(&self) -> usize {
        self.path.len()
    }

    fn normalize(&mut self, element_id: ElementId) -> ElementId {
        match element_id {
            ElementId::CodeLocation(source_location) => {
                let occurrences = self
                    .local_occurrences
                    .last_mut()
                    .expect("element id stack must have a root local scope");
                let occurrence = if let Some((_, occurrence)) = occurrences
                    .iter_mut()
                    .find(|(location, _)| *location == source_location)
                {
                    let current = *occurrence;
                    *occurrence += 1;
                    current
                } else {
                    occurrences.push((source_location, 1));
                    0
                };
                let normalized = ElementId::Local(LocalElementId::new(source_location, occurrence));
                normalized
            }
            ElementId::NamedChild(base, name) => {
                ElementId::NamedChild(Arc::new(self.normalize((*base).clone())), name)
            }
            element_id => element_id,
        }
    }

    #[cfg(debug_assertions)]
    fn record_explicit_sibling(&mut self, element_id: &ElementId) {
        if matches!(element_id, ElementId::Local(_)) {
            return;
        }

        let siblings = self
            .explicit_siblings
            .last_mut()
            .expect("element id stack must have a root sibling scope");
        debug_assert!(
            !siblings.iter().any(|sibling| sibling == element_id),
            "duplicate sibling element key: {}",
            element_id
        );
        siblings.push(element_id.clone());
    }

    #[cfg(not(debug_assertions))]
    fn record_explicit_sibling(&mut self, _element_id: &ElementId) {}

    fn push_child_scope(&mut self) {
        self.local_occurrences.push(LocalOccurrences::default());
        #[cfg(debug_assertions)]
        self.explicit_siblings.push(ExplicitSiblings::default());
        debug_assert_eq!(self.local_occurrences.len(), self.path.len() + 1);
    }
}

impl Deref for ElementIdStack {
    type Target = [ElementId];

    fn deref(&self) -> &Self::Target {
        self.path.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same_location() -> core::panic::Location<'static> {
        *core::panic::Location::caller()
    }

    #[test]
    fn code_locations_are_normalized_by_parent_occurrence() {
        let location = same_location();
        let mut stack = ElementIdStack::default();

        stack.push(ElementId::CodeLocation(location));
        assert!(matches!(&stack[0], ElementId::Local(local) if local.occurrence() == 0));
        stack.pop();

        stack.push(ElementId::CodeLocation(location));
        assert!(matches!(&stack[0], ElementId::Local(local) if local.occurrence() == 1));
    }

    #[test]
    fn local_occurrences_are_scoped_by_parent_namespace() {
        let location = same_location();
        let mut stack = ElementIdStack::default();

        stack.push("first-parent");
        stack.push(ElementId::CodeLocation(location));
        assert!(matches!(&stack[1], ElementId::Local(local) if local.occurrence() == 0));
        stack.pop();
        stack.pop();

        stack.push("second-parent");
        stack.push(ElementId::CodeLocation(location));
        assert!(matches!(&stack[1], ElementId::Local(local) if local.occurrence() == 0));
    }

    #[test]
    fn begin_pass_resets_local_occurrences_for_lifecycle_replay() {
        let location = same_location();
        let mut stack = ElementIdStack::default();

        stack.push(ElementId::CodeLocation(location));
        assert!(matches!(&stack[0], ElementId::Local(local) if local.occurrence() == 0));
        stack.pop();

        stack.begin_pass();
        stack.push(ElementId::CodeLocation(location));
        assert!(matches!(&stack[0], ElementId::Local(local) if local.occurrence() == 0));
    }

    #[test]
    fn begin_pass_allows_same_explicit_key_across_lifecycle_phases() {
        let mut stack = ElementIdStack::default();

        stack.push("same-key");
        stack.pop();
        stack.begin_pass();
        stack.push("same-key");
        stack.pop();
    }

    #[test]
    fn pop_empty_stack_preserves_root_scope() {
        let location = same_location();
        let mut stack = ElementIdStack::default();

        assert_eq!(stack.pop(), None);
        stack.push(ElementId::CodeLocation(location));

        assert!(matches!(&stack[0], ElementId::Local(local) if local.occurrence() == 0));
    }

    #[test]
    fn signed_integer_identity_rejects_negative_values() {
        assert_eq!(ElementId::try_from(7i32).unwrap(), ElementId::Integer(7));
        assert!(ElementId::try_from(-1i32).is_err());

        assert_eq!(
            ValueKey::try_from(7i32).unwrap().into_element_id(),
            ElementId::Integer(7)
        );
        assert!(ValueKey::try_from(-1i32).is_err());
    }

    #[test]
    fn key_conversions_preserve_value_and_global_identity() {
        assert_eq!(ElementId::from(Key::value("row")), ElementId::from("row"));

        let global = GlobalKey::new("app-root");
        assert_eq!(
            ElementId::from(Key::from(global.clone())),
            ElementId::GlobalKey(global)
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "duplicate sibling element key")]
    fn duplicate_explicit_sibling_key_panics_in_debug() {
        let mut stack = ElementIdStack::default();

        stack.push("same-key");
        stack.pop();
        stack.push("same-key");
    }
}
