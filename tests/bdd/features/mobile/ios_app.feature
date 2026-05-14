Feature: iOS Companion App
  The iOS companion app provides chat, voice, and canvas
  interfaces to the SmartAssist gateway.

  Scenario: App initializes with correct version
    Given the iOS SmartAssist app
    Then the app version should be "0.1.0"

  Scenario: Device model initialization
    Given a device with id "ios-1", name "iPhone", type "ios"
    Then the device id should be "ios-1"
    And the device should not be connected

  Scenario: Chat message model
    Given a chat message with role "user" and content "Hello"
    Then the message role should be "user"
    And the message content should be "Hello"

  Scenario: Session initialization
    Given a new session
    Then the session status should be "active"
    And the session should have no messages
