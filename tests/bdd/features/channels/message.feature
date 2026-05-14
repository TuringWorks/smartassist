Feature: Channel message delivery

  Scenario: Send a message through a connected channel
    Given the SmartAssist gateway is running
    And a channel named 'telegram' is connected
    When I send a message 'hello world' to the channel
    Then the response status should be 200
    And the session should receive a response

  Scenario: Receive a message from a connected channel
    Given the SmartAssist gateway is running
    And a channel named 'discord' is connected
    When a message 'ping' is received from the channel
    Then the response status should be 200
    And the session should receive a response
