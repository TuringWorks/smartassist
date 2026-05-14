Feature: Heartbeat Filter
  Heartbeat filters suppress periodic status messages and
  keep-alive traffic from channels.

  Scenario: Detect Discord ping as heartbeat
    Given a heartbeat filter with default patterns
    When a message "ping" arrives from channel "discord"
    Then the message should be classified as heartbeat

  Scenario: Normal chat message is not heartbeat
    Given a heartbeat filter with default patterns
    When a message "Hello everyone!" arrives from channel "discord"
    Then the message should not be classified as heartbeat

  Scenario: Empty message is heartbeat
    Given a heartbeat filter with default patterns
    When a message "" arrives from channel "slack"
    Then the message should be classified as heartbeat

  Scenario: Numeric status code is heartbeat
    Given a heartbeat filter with default patterns
    When a message "200" arrives from channel "web"
    Then the message should be classified as heartbeat

  Scenario: Channel-specific pattern only applies to target channel
    Given a heartbeat filter with default patterns
    When a message "ping" arrives from channel "telegram"
    Then the message should not be classified as heartbeat
