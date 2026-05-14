Feature: Channel Auto-Reply
  Auto-reply rules allow channels to respond automatically to
  incoming messages based on pattern matching.

  Scenario: Exact match auto-reply triggers
    Given an auto-reply engine with rule "greeting" exact "hello" replies "Hi there!"
    When a message "hello" is received in chat "chat1"
    Then an outbound message "Hi there!" should be generated

  Scenario: Contains match auto-reply triggers
    Given an auto-reply engine with rule "help" contains "help" replies "I can help!"
    When a message "I need help please" is received in chat "chat1"
    Then an outbound message "I can help!" should be generated

  Scenario: Regex match auto-reply triggers
    Given an auto-reply engine with rule "price" regex "\bprice\b" replies "See pricing."
    When a message "what is the price?" is received in chat "chat1"
    Then an outbound message "See pricing." should be generated

  Scenario: Disabled rule does not trigger
    Given an auto-reply engine with disabled rule "off" exact "test"
    When a message "test" is received in chat "chat1"
    Then no outbound message should be generated

  Scenario: Cooldown prevents repeat triggers
    Given an auto-reply engine with rule "spam" exact "spam" replies "stop" and cooldown 60 seconds
    When a message "spam" is received in chat "chat1"
    And a message "spam" is received in chat "chat1" again immediately
    Then only 1 outbound message should be generated
