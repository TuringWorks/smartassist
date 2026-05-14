Feature: Android Companion App
  The Android companion app provides chat, voice, and canvas
  interfaces to the SmartAssist gateway.

  Scenario: Device model initialization
    Given an Android device with id "android-1", name "Pixel", type "android"
    Then the device id should be "android-1"
    And the device should not be connected

  Scenario: Chat message model
    Given an Android chat message with role "user" and content "Hello"
    Then the message role should be "user"
    And the message content should be "Hello"

  Scenario: Session initialization
    Given a new Android session
    Then the session status should be "active"
    And the session should have no messages

  Scenario: Gateway client default URL
    Given a new GatewayClient
    Then the base URL should be "http://localhost:18789"
