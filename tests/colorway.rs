use std::collections::BTreeSet;

use gitlaxy::colorway::{Colorway, Palette};
use gitlaxy::theme::Theme;

#[test]
fn all_six_names_parse_and_the_default_is_blue() {
    for colorway in Colorway::ALL {
        assert_eq!(Colorway::parse(colorway.name()), Some(colorway));
    }
    assert_eq!(Colorway::default(), Colorway::Blue);
    assert_eq!(Colorway::parse("blue"), Some(Colorway::Blue));
    assert_eq!(Colorway::parse("Blue"), None);
    assert_eq!(Colorway::parse("teal"), None);
    assert_eq!(Colorway::parse(""), None);
}

#[test]
fn each_colorway_palette_starts_on_the_exact_theme_background() {
    for theme in [Theme::Dark, Theme::Light] {
        let background = theme.background();
        for colorway in Colorway::ALL {
            let Palette { anchors } = colorway.palette(theme);
            assert_eq!(
                anchors[0],
                [background[0], background[1], background[2]],
                "{} on {theme:?}",
                colorway.name()
            );
        }
    }
}

#[test]
fn colorways_have_distinct_palettes_in_both_themes() {
    for theme in [Theme::Dark, Theme::Light] {
        let mut palettes = BTreeSet::new();
        for colorway in Colorway::ALL {
            let Palette { anchors } = colorway.palette(theme);
            assert!(
                palettes.insert(anchors),
                "{} duplicates another palette on {theme:?}",
                colorway.name()
            );
        }
        assert_eq!(palettes.len(), Colorway::ALL.len());
    }
}
