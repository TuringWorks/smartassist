Feature: Internationalization
  SmartAssist provides translated user-facing messages through
  a lightweight i18n system backed by YAML locale files.

  Scenario: Translate a welcome message
    Given the locale is set to "en"
    When translating key "welcome"
    Then the result should contain "SmartAssist"

  Scenario: Translate with interpolation
    Given the locale is set to "en"
    When translating key "error_agent_not_found" with variable "agent" = "mybot"
    Then the result should contain "mybot"

  Scenario: Fallback to key for missing translation
    Given the locale is set to "en"
    When translating key "nonexistent_key_12345"
    Then the result should be "nonexistent_key_12345"
