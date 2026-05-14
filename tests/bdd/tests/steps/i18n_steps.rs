//! Internationalization step definitions.

use cucumber::{given, then, when};
use crate::SmartAssistWorld;

#[given(regex = r#"^the locale is set to "(\w+)"$"#)]
async fn set_locale(world: &mut SmartAssistWorld, locale: String) {
    smartassist_core::i18n::set_locale(&locale);
    world.current_locale = Some(locale);
}

#[when(regex = r#"^translating key "([^"]*)"$"#)]
async fn translating_key(world: &mut SmartAssistWorld, key: String) {
    world.last_translation = Some(smartassist_core::i18n::t(&key));
}

#[when(regex = r#"^translating key "(.*)" with variable "(\w+)" = "(.*)"$"#)]
async fn translating_key_with_var(
    world: &mut SmartAssistWorld,
    key: String,
    var: String,
    value: String,
) {
    world.last_translation = Some(smartassist_core::i18n::t_with(&key, &var, &value));
}

#[then(regex = r#"^the result should contain "(.*)"$"#)]
async fn result_contains(world: &mut SmartAssistWorld, expected: String) {
    let result = world.last_translation.as_ref().expect("no translation result");
    assert!(
        result.contains(&expected),
        "expected result to contain '{}', got: {}",
        expected,
        result
    );
}

#[then(regex = r#"^the result should be "(.*)"$"#)]
async fn result_is(world: &mut SmartAssistWorld, expected: String) {
    let result = world.last_translation.as_ref().expect("no translation result");
    assert_eq!(
        result, &expected,
        "expected result '{}', got: {}",
        expected,
        result
    );
}
