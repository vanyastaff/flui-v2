#[test]
fn test_derive_render() {
    use flui_macros::Render;

    #[derive(Render)]
    struct _Element;
}
