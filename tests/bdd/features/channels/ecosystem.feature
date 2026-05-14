Feature: Channel ecosystem expansion

  Scenario: IRC channel can be created and connected
    Given the SmartAssist gateway is running
    And a channel named 'irc' is connected
    When I send a message 'hello irc' to the channel
    Then the response status should be 200

  Scenario: Matrix channel can be created and connected
    Given the SmartAssist gateway is running
    And a channel named 'matrix' is connected
    When I send a message 'hello matrix' to the channel
    Then the response status should be 200

  Scenario: Google Chat channel can be created and connected
    Given the SmartAssist gateway is running
    And a channel named 'googlechat' is connected
    When I send a message 'hello googlechat' to the channel
    Then the response status should be 200

  Scenario: Microsoft Teams channel can be created and connected
    Given the SmartAssist gateway is running
    And a channel named 'msteams' is connected
    When I send a message 'hello teams' to the channel
    Then the response status should be 200

  Scenario: Feishu channel can be created and connected
    Given the SmartAssist gateway is running
    And a channel named 'feishu' is connected
    When I send a message 'hello feishu' to the channel
    Then the response status should be 200

  Scenario: Mattermost channel can be created and connected
    Given the SmartAssist gateway is running
    And a channel named 'mattermost' is connected
    When I send a message 'hello mattermost' to the channel
    Then the response status should be 200

  Scenario: Zalo channel can be created and connected
    Given the SmartAssist gateway is running
    And a channel named 'zalo' is connected
    When I send a message 'hello zalo' to the channel
    Then the response status should be 200
