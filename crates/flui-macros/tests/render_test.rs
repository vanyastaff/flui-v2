#[test]
fn test_derive_render() {
    use flui_macros::Render;

    #[derive(Render)]
    struct _Element;
}

#[test]
fn test_derive_into_element_still_targets_render_once_component() {
    use flui_core::{IntoElement as _, RenderOnce};
    use flui_macros::IntoElement;

    #[derive(IntoElement)]
    struct Recipe;

    impl RenderOnce for Recipe {
        fn render(
            self,
            _window: &mut flui_core::Window,
            _cx: &mut flui_core::App,
        ) -> impl flui_core::IntoElement {
            flui_core::Empty
        }
    }

    fn assert_component<T: RenderOnce>(_: flui_core::Component<T>) {}

    assert_component(Recipe.into_element());
}
