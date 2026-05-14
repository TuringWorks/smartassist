Feature: Voice talk sessions

  Scenario: Start a voice session
    Given the SmartAssist gateway is running
    When I start a talk session
    Then the response should contain 'started'
    And the response should contain 'session_id'

  Scenario: Stop a voice session
    Given the SmartAssist gateway is running
    And I have an active talk session
    When I stop the talk session
    Then the response should contain 'stopped'

  Scenario: Push-to-talk press and release
    Given the SmartAssist gateway is running
    And I have an active talk session
    When I press push-to-talk
    Then the response should contain 'ptt_pressed'
    When I release push-to-talk after 500 ms
    Then the response should contain 'ptt_released'

  Scenario: Send audio to a talk session
    Given the SmartAssist gateway is running
    And I have an active talk session
    When I send base64 audio to the talk session
    Then the response should contain 'received'

  Scenario: Get talk session status
    Given the SmartAssist gateway is running
    And I have an active talk session
    When I request talk session status
    Then the response should contain 'session_id'
    And the response should contain 'state'

  Scenario: List active talk sessions
    Given the SmartAssist gateway is running
    And I have an active talk session
    When I list talk sessions
    Then the response should contain 'sessions'
    And the response should contain 'count'
