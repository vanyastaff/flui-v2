#[test]
fn test_derive_context() {
    use flui_core::{App, Window};
    use flui_macros::{AppContext, VisualContext};

    #[derive(AppContext, VisualContext)]
    struct _MyCustomContext<'a, 'b> {
        #[app]
        app: &'a mut App,
        #[window]
        window: &'b mut Window,
    }
}
